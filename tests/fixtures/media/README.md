# Media API Response Fixtures

This directory contains examples of Deezer's media streaming URL API requests and responses that have been thoroughly anonymized.

**IMPORTANT - ANONYMIZED DATA**

All data has been deliberately anonymized with clearly artificial values:
- License and track tokens use 'x' patterns while maintaining the correct length
- URLs have been modified to mask CDN endpoints and paths
- Query parameters and authentication data are replaced with 'x' placeholders
- Timestamps are preserved to show temporal relationships

## File Overview

- `request.json`: Example request containing:
  - License token for authentication
  - Requested media formats and cipher types
  - Track tokens for content identification
- `response.json`: Example response containing:
  - CDN URLs for content delivery
  - Format-specific streaming endpoints
  - Token expiration data (nbf/exp timestamps)

## Response Characteristics

The response contains:
- Multiple CDN source URLs per format for redundancy
- Provider identifiers for CDN selection
- URL authentication parameters
- NBF (Not Before) and expiration timestamps
- Format and cipher type indicators

### Important Notes

- All URLs are completely masked to protect CDN infrastructure
- Authentication tokens and parameters use obvious placeholder patterns
- Multiple CDN providers are listed for failover
- Supported formats include FLAC and various MP3 bitrates
- All cipher type information is preserved while masking implementation details

## Purpose

These fixtures document the structure of media streaming URL requests and responses while using clearly artificial values. They are provided for development and testing purposes only and cannot be used for actual content access.
