# User Radio API Response Fixtures

This directory contains examples of Deezer's user radio/flow API responses that have been thoroughly anonymized.

**IMPORTANT - ANONYMIZED DATA**

All data in these files has been deliberately anonymized with clearly artificial values:
- Track, album, and artist IDs use simple placeholder numbers
- Media tokens and file hashes use 'x' patterns to maintain length but be obviously fake
- URLs have been modified to use 'x' placeholders
- Track metadata like titles uses Lorem Ipsum text
- File sizes and audio characteristics maintain realistic proportions but are example values

## File Overview

- `request.json`: Example request payload with user ID
- `response.json`: Example response with radio tracks

## Response Characteristics

The response contains:
- A list of track objects with detailed metadata
- Each track includes streaming rights, file formats/sizes, and tokens
- Additional payload data with algorithmic scores and genre predictions
- Metrics about track popularity and user affinity

### Important Notes

- The example response has been truncated to 2 tracks for brevity (typical responses contain 24 tracks)
- The structure and relationships between fields are preserved
- All identifiers and secured content use obvious placeholder patterns
- File sizes maintain realistic proportions between formats (MP3 128/320/FLAC etc)

## Purpose

These fixtures document the structure and relationships in the user radio/flow API responses while using clearly non-realistic values. They are provided for development and testing purposes only and cannot be used for actual streaming or content access.
