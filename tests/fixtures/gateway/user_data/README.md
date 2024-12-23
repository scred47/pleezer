# User Data API Response Fixtures

This directory contains examples of Deezer user data API responses that have been thoroughly anonymized.

**IMPORTANT - ANONYMIZED DATA**

All data in these files has been completely anonymized:
- All tokens, IDs, and authentication data are fake and cannot be used to authenticate
- Personal information has been replaced with dummy data
- Device identifiers have been replaced with example values
- All unique identifiers have been changed to non-existent values

**Any attempt to use this data for authentication or user identification would be futile - these are reference examples only.**

## File Overview

- `free.json`: Response for a free-tier account that has never used the web player
- `family_hifi.json`: Response for a Family HiFi subscription with web player history

## Response Characteristics

The responses vary in several ways:
- Different fields are present depending on the user's subscription level
- Additional fields appear if the user has previously used the web player
- Some fields may be empty or missing entirely depending on the user's history with the service

### Important Notes

- A `USER_ID` of "0" indicates no user is logged in (invalid session)
- All identifiers, tokens, and personal information in these fixtures have been anonymized while maintaining the original format and structure
- The structure of the responses is authentic, but all values are synthetic

## Purpose

These fixtures document the structure and variations of the Deezer user data API responses, helping developers understand the possible fields and values they might encounter when implementing API integrations. They are provided for reference only and cannot be used for actual authentication or user identification.
