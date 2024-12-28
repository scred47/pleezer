# List Data API Response Fixtures

This directory contains examples of Deezer's list data API requests and responses for different content types, thoroughly anonymized.

**IMPORTANT - ANONYMIZED DATA**

All data has been deliberately anonymized with clearly artificial values:
- Content IDs use simple placeholder numbers
- Media tokens and hashes use 'x' patterns
- URLs have been modified to mask real endpoints
- Text content uses Lorem Ipsum placeholders
- File sizes maintain realistic proportions but are example values

## File Overview

### Requests
- `episodes.json`: Request for podcast episode data by IDs (truncated to 2 IDs)
- `livestream.json`: Request for livestream data with codec preferences
- `songs/deezer.json`: Request for Deezer track data by IDs
- `songs/uploaded.json`: Request for user-uploaded track data by IDs (truncated to 2 IDs)

### Responses
- `episodes.json`: Podcast episode metadata and stream tokens (truncated to 2 episodes, typically returns all episodes)
- `livestream.json`: Livestream metadata and URLs for different quality levels
- `songs/deezer.json`: Deezer track metadata with streaming rights and file formats
- `songs/uploaded.json`: User-uploaded track metadata with simplified format (truncated to 2 tracks)

## Response Characteristics

The responses contain:
- Detailed metadata appropriate for each content type
- Streaming rights and availability information
- File format options and sizes where applicable
- Media tokens and URLs (anonymized)
- Content type indicators

### Important Notes

- Uploaded songs use negative IDs to distinguish them from Deezer content
- Livestreams provide URLs for different quality/codec combinations
- Episodes include show metadata along with episode details
- All secure tokens and URLs use obvious placeholder patterns
- Examples with multiple IDs have been truncated to 2 items for brevity, though real requests/responses typically include many more items

## Purpose

These fixtures document the structure and relationships in the list data API responses while using clearly non-realistic values. They are provided for development and testing purposes only and cannot be used for actual content access.
