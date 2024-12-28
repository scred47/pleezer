# pleezer Test Fixtures

This directory contains anonymized API request/response fixtures for testing Deezer API integrations. All sensitive data has been deliberately masked while preserving the correct data structures.

## Directory Structure

### Gateway API (`/gateway`)
Core gateway endpoint fixtures:

- [ARL (Authentication)](gateway/arl/README.md) - Access Request License token generation
- [List Data](gateway/list_data/README.md) - Batch content fetching (episodes, livestreams, tracks)
- [User Data](gateway/user_data/README.md) - User profile and preferences
- [User Radio](gateway/user_radio/README.md) - Personalized radio/flow recommendations

### Media (`/media`)
- [Documentation](media/README.md)
- CDN streaming URL generation
- Format and cipher type handling
- Multi-CDN failover support

### Authentication (`auth.json`)
Simple authentication response containing:
- Access token (anonymized)
- Expiration timestamps
- Multi-account status flag

## Common Anonymization Patterns

All fixtures follow these consistent anonymization principles:
- IDs use simple placeholder numbers
- Tokens/hashes use 'x' patterns while maintaining length
- URLs are masked with 'x' placeholders
- Text content uses Lorem Ipsum where appropriate
- File sizes maintain realistic proportions
- All authentication and secure data is obviously non-functional

## Truncated Responses

Several response fixtures have been truncated for brevity while preserving key structures:
- List Data responses show 2 items instead of full lists
- User Radio shows 2 tracks instead of typical 24
- Media responses show single format instead of all requested formats
- All key data structures and relationships are preserved

## Purpose

These fixtures document the structure and relationships of Deezer's API responses while using clearly artificial values. They are provided for development and testing purposes only and cannot be used for actual authentication or content access.

Each endpoint's individual README provides more detailed information about its specific request/response characteristics.

## Security Note

All sensitive data has been thoroughly anonymized:
- No real authentication tokens or credentials
- No actual CDN infrastructure details
- No personally identifiable information
- No real user data or preferences
- All tokens and secure IDs are clearly artificial

The fixtures maintain the correct data structures and relationships while ensuring no sensitive information is exposed.
