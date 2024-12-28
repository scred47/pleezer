# Deezer Gateway API Fixtures

This directory contains anonymized request/response fixtures for various Deezer gateway API endpoints. All fixtures have been thoroughly anonymized to protect security while maintaining the correct data structures.

## Available Endpoints

### ARL (Access Request License)
- [Documentation](arl/README.md)
- Handles authentication token generation
- Example of server-generated credential response

### List Data
- [Documentation](list_data/README.md)
- Batch fetching of different content types:
  - Podcast episodes
  - Livestream metadata
  - Deezer tracks
  - User-uploaded tracks
- Examples show truncated responses (2 items) for brevity

### User Data
- [Documentation](user_data/README.md)
- User profile and preferences data
- Examples for both free and premium (family) accounts
- Includes subscription details, settings, and feature flags

### User Radio
- [Documentation](user_radio/README.md)
- Personalized radio/flow recommendations
- Includes algorithmic scoring and track metadata
- Example shows truncated response (2 tracks vs typical 24)

## Common Characteristics

All fixtures share these anonymization patterns:
- IDs use simple placeholder numbers
- Tokens/hashes use 'x' patterns while maintaining length
- URLs are masked with 'x' placeholders
- Text content uses Lorem Ipsum where appropriate
- File sizes maintain realistic proportions
- All authentication and secure data is obviously non-functional

## Purpose

These fixtures document the structure and relationships of Deezer's gateway API responses while using clearly artificial values. They are provided for development and testing purposes only and cannot be used for actual authentication or content access.

Each endpoint's individual README provides more detailed information about its specific request/response characteristics.
