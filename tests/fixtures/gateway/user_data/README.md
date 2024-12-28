# User Data API Response Fixtures

This directory contains examples of Deezer user data API responses that have been thoroughly anonymized.

**IMPORTANT - ANONYMIZED DATA**

All data in these files has been deliberately anonymized with clearly artificial values:
- All tokens and IDs use obvious placeholder patterns (e.g., "xxxxx")
- Authentication data has been replaced with non-realistic values
- Personal information uses generic placeholder data (e.g., "john.doe@example.com")
- Device identifiers and unique IDs use obviously fake patterns
- Sensitive keys and tokens are masked with 'x' characters while maintaining length patterns

**These fixtures are intentionally constructed to be obviously non-functional and cannot be used for authentication.**

## File Overview

- `free.json`: Response for a free-tier account that has never used the web player
- `family.json`: Response for a Family subscription with web player history

## Response Characteristics

The responses vary in several ways:
- Different fields are present depending on the user's subscription level
- Additional fields appear if the user has previously used the web player
- Some fields may be empty or missing entirely depending on the user's history with the service

### Important Notes

- A `USER_ID` of "0" indicates no user is logged in (invalid session)
- All identifiers and tokens use obvious placeholder patterns (xxxxx) to clearly indicate they are non-functional
- The structure of the responses is authentic, but all values are intentionally artificial

## Purpose

These fixtures document the structure and variations of the Deezer user data API responses for development and testing purposes only. They maintain the format and structure of real responses while using clearly non-realistic values to prevent any potential misuse.
