use std::{
    collections::HashMap,
    fmt::{self, Write},
};

use serde::{de::Error, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use unescape::unescape;

#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct RemoteMessage(String, String, RemoteContents);

impl RemoteMessage {
    pub fn new(
        message_type: &str,
        from: Option<u64>,
        destination: Option<u64>,
        app: &str,
        app_id: Option<u64>,
        contents: RemoteContents,
    ) -> Result<Self, fmt::Error> {
        let mut namespace = String::new();
        if let Some(id) = from {
            write!(namespace, "{id}")?;
        } else {
            write!(namespace, "-1")?;
        }
        write!(namespace, "_")?;
        if let Some(id) = destination {
            write!(namespace, "{id}")?;
        } else {
            write!(namespace, "-1")?;
        }
        write!(namespace, "_{app}")?;
        if let Some(id) = app_id {
            write!(namespace, "_{id}")?;
        }

        Ok(Self(message_type.to_owned(), namespace, contents))
    }

    pub fn message_type(&self) -> &str {
        &self.0
    }

    pub fn from(&self) -> Option<u64> {
        let id = self.1.split('_').nth(1)?;

        // The string may contain `-1` meaning: unknown.
        return str::parse::<u64>(id).map_or(None, |id| Some(id));
    }

    pub fn destination(&self) -> Option<u64> {
        let id = self.1.split('_').nth(2)?;
        str::parse::<u64>(id).map_or(None, |id| Some(id))
    }

    pub fn app(&self) -> Option<&str> {
        self.1.split('_').nth(3)
    }

    pub fn app_id(&self) -> Option<u64> {
        let id = self.1.split('_').nth(4)?;
        str::parse::<u64>(id).map_or(None, |id| Some(id))
    }

    pub fn contents(&self) -> &RemoteContents {
        &self.2
    }
}

#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct RemoteContents {
    #[serde(rename = "APP")]
    pub app: String,
    pub headers: RemoteHeader,
    pub body: String,
}

impl RemoteContents {
    pub fn body(&self) -> Result<RemoteBody, serde_json::Error> {
        let body_str =
            unescape(&self.body).ok_or_else(|| serde_json::Error::custom("unescape failed"))?;
        serde_json::from_str(&body_str)
    }
}

#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct RemoteHeader {
    pub from: String,
    pub destination: Option<String>,
}

#[derive(Clone, Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBody {
    pub message_id: String,
    pub message_type: String,
    pub protocol_version: String,
    payload: Option<Base64>,

    // Seems always empty.
    pub clock: HashMap<String, Value>,
}

impl RemoteBody {
    pub fn payload(&self) -> Result<RemotePayload, serde_json::Error> {
        match &self.payload {
            Some(payload) => {
                if self.message_type != "playbackQueue" {
                    serde_json::from_slice(payload.as_ref())
                } else {
                    unimplemented!()
                }
            }
            None => Err(serde_json::Error::custom("payload empty")),
        }
    }
}

#[derive(Clone, Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum RemotePayload {
    Ack {
        acknowledgement_id: String,
    },
    PlaybackProgress {
        queue_id: String,
        element_id: String,
        progress: f64,
        buffered: i64,
        duration: i64,
        quality: i64,
        volume: f64,
        is_playing: bool,
        is_shuffle: bool,
        repeat_mode: i64,
    },
    //PlaybackQueue {
    // TODO: from proto
    //}
    PlaybackStatus {
        command_id: String,
        status: i64,
    },
    Skip {
        element_id: String,
        progress: f64,
        queue_id: String,
        set_repeat_mode: i64,
        should_play: bool,
    },
    WithParams {
        from: String,
        params: RemoteParams,
    },
}

#[derive(Clone, Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum RemoteParams {
    ConnectionOffer {
        device_name: String,
        device_type: String,
        supported_control_version: Vec<String>,
    },
    DiscoveryRequest {
        discovery_session: String,
    },
}

#[derive(Clone, Debug)]
pub struct Base64(Vec<u8>);

impl AsRef<[u8]> for Base64 {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Display for Base64 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let utf8 = std::str::from_utf8(&self.0).unwrap_or("binary");
        write!(f, "{utf8}")
    }
}

impl Serialize for Base64 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&base64::display::Base64Display::with_config(
            &self.0,
            base64::STANDARD,
        ))
    }
}

impl<'de> Deserialize<'de> for Base64 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Base64Visitor;
        impl serde::de::Visitor<'_> for Base64Visitor {
            type Value = Base64;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a base64 string")
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                base64::decode(v)
                    .map(Base64)
                    .map_err(serde::de::Error::custom)
            }
        }
        deserializer.deserialize_str(Base64Visitor)
    }
}

// struct Base64Visitor;
//
// impl<'de> Visitor<'de> for PayloadVisitor {
//     type Value = RemotePayload;
//
//     fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
//         formatter.write_str("a UTF-8 string encoded as base64")
//     }
//
//     fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
//     where
//         E: de::Error,
//     {
//         let decoded = base64::decode(value)
//             .map_err(|e| E::invalid_type(Unexpected::Str(value), &self))?;
//         std::str::from_utf8(&decoded).map_err(|e| E::invalid_value(Unexpected::Bytes(&decoded), &self))
//     }
// }
//
// fn de_base64_json<'de, D>(deserializer: D) -> Result<Option<RemotePayload>, D::Error>
// where
//     D: Deserializer<'de>,
// {
//     deserializer.deserialize_any(PayloadVisitor).ok()
// }

// ["msg","4787654542_4787654542_REMOTEDISCOVER",{"APP":"REMOTEDISCOVER","body":"{\"messageId\":\"56E10C0A-ABF2-4D1C-BF12-80858AFB1AE7\",\"protocolVersion\":\"com.deezer.remote.discovery.proto1\",\"payload\":\"eyJmcm9tIjoieTRhOTcxZWNmMTFjOTE2ZjY2MWUxMzI4ZDM1YWY2NWQ3IiwicGFyYW1zIjp7ImRpc2NvdmVyeV9zZXNzaW9uIjoiRUI5MDRBM0UtNTc3RS00QTI1LTlGNEItOTA5RkY2RUMwMDVDIn19\",\"messageType\":\"discoveryRequest\",\"clock\":{}}","headers":{"from":"y4a971ecf11c916f661e1328d35af65d7"}}]
//
//
// ["send","4787654542_4787654542_REMOTEDISCOVER",{"APP":"REMOTEDISCOVER","headers":{"from":"489b2ced-dc5d-4571-a839-40d30af980ef","destination":"y4a971ecf11c916f661e1328d35af65d7"},"body":"{\"messageId\":\"69abdd74-6b2e-44da-abe7-f4dd46a9cbdf\",\"messageType\":\"connectionOffer\",\"protocolVersion\":\"com.deezer.remote.discovery.proto1\",\"clock\":{},\"payload\":\"eyJmcm9tIjoiNDg5YjJjZWQtZGM1ZC00NTcxLWE4MzktNDBkMzBhZjk4MGVmIiwicGFyYW1zIjp7ImRldmljZV9uYW1lIjoiUm9kZXJpY2tzLWlNYWMtMy5sb2NhbCIsImRldmljZV90eXBlIjoid2ViIiwic3VwcG9ydGVkX2NvbnRyb2xfdmVyc2lvbnMiOlsiMS4wLjAtYmV0YTIiXX19\"}"}]
//
// ["sub","-1_4787654542_USERFEED_4787654542"]
//
// ["sub","4787654542_4787654542_REMOTEDISCOVER"]
//
// ["sub","4787654542_4787654542_REMOTEQUEUE"]
// ["unsub","4787654542_4787654542_REMOTEQUEUE"]
//
// ["sub","4787654542_4787654542_REMOTECOMMAND"]
// ["unsub","4787654542_4787654542_REMOTECOMMAND"]
// ["send","4787654542_4787654542_REMOTECOMMAND",{"APP":"REMOTECOMMAND","headers":{"from":"489b2ced-dc5d-4571-a839-40d30af980ef","destination":"y4a971ecf11c916f661e1328d35af65d7"},"body":"{\"messageId\":\"1b14d2bc-aa25-4f2e-8fbc-1daa7f94b399\",\"messageType\":\"ready\",\"protocolVersion\":\"com.deezer.remote.command.proto1\",\"clock\":{},\"payload\":\"\"}"}]
//
// FOR OURSELVES: filter out (echoed)
// ["msg","4787654542_4787654542_REMOTECOMMAND",{"APP":"REMOTECOMMAND","body":"{\"messageId\":\"1b14d2bc-aa25-4f2e-8fbc-1daa7f94b399\",\"messageType\":\"ready\",\"protocolVersion\":\"com.deezer.remote.command.proto1\",\"clock\":{},\"payload\":\"\"}","headers":{"destination":"y4a971ecf11c916f661e1328d35af65d7","from":"489b2ced-dc5d-4571-a839-40d30af980ef"}}]
//
// acked by peer:
// ["msg","4787654542_4787654542_REMOTECOMMAND",{"APP":"REMOTECOMMAND","body":"{\"messageId\":\"8FC19B8A-B9DD-4F50-AF22-FC403AE0C657\",\"protocolVersion\":\"com.deezer.remote.command.proto1\",\"payload\":\"eyJhY2tub3dsZWRnZW1lbnRJZCI6IjFiMTRkMmJjLWFhMjUtNGYyZS04ZmJjLTFkYWE3Zjk0YjM5OSJ9\",\"messageType\":\"ack\",\"clock\":{}}","headers":{"destination":"489b2ced-dc5d-4571-a839-40d30af980ef","from":"y4a971ecf11c916f661e1328d35af65d7"}}]
//
// queue published by server:
// ["msg","4787654542_4787654542_REMOTEQUEUE",{"APP":"REMOTEQUEUE","body":"{\"messageId\":\"BA1C7A7A-BA8E-41A7-B0C0-E84CE754DAA5\",\"protocolVersion\":\"com.deezer.remote.queue.proto1\",\"payload\":\"hVTNjuM2DAaKYrrr9lDMpcCcu7d4VxIpkTrksIOdvkbgOMqMUf8EttNFeu0r9NRL36Iv1XcoSknOZHcQYJAgMPVR5Ed+n1P87Plef1L3utT3jksEMOVHfHClffhFsdIPmj893P5zU9wWBRKTs2jR\\/Pjt3Tdvitu\\/b4o\\/bg7VY9g0u\\/XT0IXVGPpdGGNYGQTv6tpqy1sbdAhhb63Tuy1sCbxd\\/RbGqRn6df1U9X1opw+xQPopu2HbtKE8jKFrjt17\\/V6tplDPkp0bjc3vQz9Xbfk4Nrtn6DBMTXxY46obdsc2sTLbbQ20d6W3uC+R1L7c1rgt1d5v\\/b6GSvv6nD6fDmE9ddU4z2NV\\/9o207xqq9NwnF+cbvLp5iWTWp5DP2+m4TjWL0ttFrTM6GYM9dB1sq8qkp5eXJbsYz+vtX4+P4zD41h1XdM\\/LuBlK0vOet8On9\\/Bx4tQ7+D+axaCNv10aMawK7enUr+W8GoFeC0BX0uw1xI6meipPZXzcLgG75qpHsRBp2tgHz6XY2hDNYXpGr6vuqa9erN+atp21cyhy2aI+8yhWOmy1Xz07Dd1+9Pu1EvRepNehuMkr8BY7Zrh7oei0Naw9469SRGAJoeKc2Q1KXAgWFG8cV6DR4KEMIPViIpSJAh4ZS3dfV+81cYqD6QwBkxKE2upEBFE76xYPQXKazZeqRgQMlqFLrdlr4yGF8U93b0tviNHVhuf2YGV0FOqLZOzitPHQE4dKEW5XLxiiTI7ii8+MiwDkmLZQB7Qg\\/MyZKaqtHWGOLFjZQzJX0\\/K0igjGNCpqWUpq1xmQN4bCfN9QJnTkEubs0ohkE4tjVRzjl0uZrUCpMTMgiG0njPg0cnqdOJFxgqBvDWjolbIywqtkMaEELAFcYCJawINjMDpNkrV5Tm2M+CzhOg9E7plE7JUZOUoXyfLQisBpKWqwXNk0QtJlxh7Ej2dWcwie5CPWCZrGOP4TaM5EG6oLnpoPifK9GBdFCQ2jpaCNJxXnj3zYgmRyYiki6IgPYX+2S6plVZfuk8n4RDRiiT2S2\\/L5SyQcUb+8flibecXANko5WIALN51sracZQDZ2ewccTZpnd1CSXs+VxZJ5aVyC3ESl5My6s\\/\\/\\/v3r5n8=\",\"messageType\":\"publishQueue\",\"clock\":{}}","headers":{"destination":"489b2ced-dc5d-4571-a839-40d30af980ef","from":"y4a971ecf11c916f661e1328d35af65d7"}}]
//
// playlist position indicated by server:
// ["msg","4787654542_4787654542_REMOTECOMMAND",{"APP":"REMOTECOMMAND","body":"{\"messageId\":\"B5B79102-B7CD-4C1D-9044-8AD26FFDCD0C\",\"protocolVersion\":\"com.deezer.remote.command.proto1\",\"payload\":\"eyJlbGVtZW50SWQiOiI5OEIxRDBCMS0xQjY4LTQzMzItQTRFNi01RUYwODAxRTE4REUtMTU4NzA3MTExMi02MyIsInByb2dyZXNzIjowLCJxdWV1ZUlkIjoiOThCMUQwQjEtMUI2OC00MzMyLUE0RTYtNUVGMDgwMUUxOERFIiwic2V0UmVwZWF0TW9kZSI6MCwic2hvdWxkUGxheSI6ZmFsc2V9\",\"messageType\":\"skip\",\"clock\":{}}","headers":{"destination":"489b2ced-dc5d-4571-a839-40d30af980ef","from":"y4a971ecf11c916f661e1328d35af65d7"}}]
//
// playback status provided by server:
// ["msg","4787654542_4787654542_REMOTECOMMAND",{"APP":"REMOTECOMMAND","body":"{\"messageId\":\"CA1C0543-C49E-40BA-A2B3-16382B57F180\",\"protocolVersion\":\"com.deezer.remote.command.proto1\",\"payload\":\"eyJjb21tYW5kSWQiOiIxYjE0ZDJiYy1hYTI1LTRmMmUtOGZiYy0xZGFhN2Y5NGIzOTkiLCJzdGF0dXMiOjB9\",\"messageType\":\"status\",\"clock\":{}}","headers":{"destination":"489b2ced-dc5d-4571-a839-40d30af980ef","from":"y4a971ecf11c916f661e1328d35af65d7"}}]
//
// everything acked by us:
// ["send","4787654542_4787654542_REMOTECOMMAND",{"APP":"REMOTECOMMAND","headers":{"from":"489b2ced-dc5d-4571-a839-40d30af980ef","destination":"y4a971ecf11c916f661e1328d35af65d7"},"body":"{\"messageId\":\"0fac32e4-02e4-4ff1-a201-45a758ada5ad\",\"messageType\":\"ack\",\"protocolVersion\":\"com.deezer.remote.command.proto1\",\"clock\":{},\"payload\":\"eyJhY2tub3dsZWRnZW1lbnRJZCI6IkI1Qjc5MTAyLUI3Q0QtNEMxRC05MDQ0LThBRDI2RkZEQ0QwQyJ9\"}"}]
//
// playback progress provided by us:
// ["send","4787654542_4787654542_REMOTECOMMAND",{"APP":"REMOTECOMMAND","headers":{"from":"489b2ced-dc5d-4571-a839-40d30af980ef","destination":"y4a971ecf11c916f661e1328d35af65d7"},"body":"{\"messageId\":\"5e311559-ddd8-4ea7-a7f7-6d82d11496ff\",\"messageType\":\"playbackProgress\",\"protocolVersion\":\"com.deezer.remote.command.proto1\",\"clock\":{},\"payload\":\"eyJxdWV1ZUlkIjoiOThCMUQwQjEtMUI2OC00MzMyLUE0RTYtNUVGMDgwMUUxOERFIiwiZWxlbWVudElkIjoiOThCMUQwQjEtMUI2OC00MzMyLUE0RTYtNUVGMDgwMUUxOERFLTE1ODcwNzExMTItNjMiLCJwcm9ncmVzcyI6MCwiYnVmZmVyZWQiOjU3OSwiZHVyYXRpb24iOjU3OSwicXVhbGl0eSI6Mywidm9sdW1lIjowLjQ2LCJpc1BsYXlpbmciOmZhbHNlLCJpc1NodWZmbGUiOmZhbHNlLCJyZXBlYXRNb2RlIjowfQ==\"}"}]
//
// playback status provided by us:
// ["send","4787654542_4787654542_REMOTECOMMAND",{"APP":"REMOTECOMMAND","headers":{"from":"489b2ced-dc5d-4571-a839-40d30af980ef","destination":"y4a971ecf11c916f661e1328d35af65d7"},"body":"{\"messageId\":\"f363c744-ba15-4dde-b9a0-50e8b07d72d1\",\"messageType\":\"status\",\"protocolVersion\":\"com.deezer.remote.command.proto1\",\"clock\":{},\"payload\":\"eyJjb21tYW5kSWQiOiJCNUI3OTEwMi1CN0NELTRDMUQtOTA0NC04QUQyNkZGRENEMEMiLCJzdGF0dXMiOjB9\"}"}]
//
// acked by server:
// ["msg","4787654542_4787654542_REMOTECOMMAND",{"APP":"REMOTECOMMAND","body":"{\"messageId\":\"0fac32e4-02e4-4ff1-a201-45a758ada5ad\",\"messageType\":\"ack\",\"protocolVersion\":\"com.deezer.remote.command.proto1\",\"clock\":{},\"payload\":\"eyJhY2tub3dsZWRnZW1lbnRJZCI6IkI1Qjc5MTAyLUI3Q0QtNEMxRC05MDQ0LThBRDI2RkZEQ0QwQyJ9\"}","headers":{"destination":"y4a971ecf11c916f661e1328d35af65d7","from":"489b2ced-dc5d-4571-a839-40d30af980ef"}}]
//
// playback progress provided by server:
// ["msg","4787654542_4787654542_REMOTECOMMAND",{"APP":"REMOTECOMMAND","body":"{\"messageId\":\"5e311559-ddd8-4ea7-a7f7-6d82d11496ff\",\"messageType\":\"playbackProgress\",\"protocolVersion\":\"com.deezer.remote.command.proto1\",\"clock\":{},\"payload\":\"eyJxdWV1ZUlkIjoiOThCMUQwQjEtMUI2OC00MzMyLUE0RTYtNUVGMDgwMUUxOERFIiwiZWxlbWVudElkIjoiOThCMUQwQjEtMUI2OC00MzMyLUE0RTYtNUVGMDgwMUUxOERFLTE1ODcwNzExMTItNjMiLCJwcm9ncmVzcyI6MCwiYnVmZmVyZWQiOjU3OSwiZHVyYXRpb24iOjU3OSwicXVhbGl0eSI6Mywidm9sdW1lIjowLjQ2LCJpc1BsYXlpbmciOmZhbHNlLCJpc1NodWZmbGUiOmZhbHNlLCJyZXBlYXRNb2RlIjowfQ==\"}","headers":{"destination":"y4a971ecf11c916f661e1328d35af65d7","from":"489b2ced-dc5d-4571-a839-40d30af980ef"}}]
//
// status provided by server:
// ["msg","4787654542_4787654542_REMOTECOMMAND",{"APP":"REMOTECOMMAND","body":"{\"messageId\":\"f363c744-ba15-4dde-b9a0-50e8b07d72d1\",\"messageType\":\"status\",\"protocolVersion\":\"com.deezer.remote.command.proto1\",\"clock\":{},\"payload\":\"eyJjb21tYW5kSWQiOiJCNUI3OTEwMi1CN0NELTRDMUQtOTA0NC04QUQyNkZGRENEMEMiLCJzdGF0dXMiOjB9\"}","headers":{"destination":"y4a971ecf11c916f661e1328d35af65d7","from":"489b2ced-dc5d-4571-a839-40d30af980ef"}}]
//
// ack by server:
// ["msg","4787654542_4787654542_REMOTECOMMAND",{"APP":"REMOTECOMMAND","body":"{\"messageId\":\"EADA5234-FABD-4E1A-9829-ABC9985F784D\",\"protocolVersion\":\"com.deezer.remote.command.proto1\",\"payload\":\"eyJhY2tub3dsZWRnZW1lbnRJZCI6IjVlMzExNTU5LWRkZDgtNGVhNy1hN2Y3LTZkODJkMTE0OTZmZiJ9\",\"messageType\":\"ack\",\"clock\":{}}","headers":{"destination":"489b2ced-dc5d-4571-a839-40d30af980ef","from":"y4a971ecf11c916f661e1328d35af65d7"}}]
// (are these echoes?)
