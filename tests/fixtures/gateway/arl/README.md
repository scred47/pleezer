# ARL (Access Request License) API Response Fixtures

This directory contains examples of Deezer's ARL token API responses that have been thoroughly anonymized.

**IMPORTANT - ANONYMIZED DATA**

The ARL token has been deliberately anonymized:
- The real token is a 192 character credential string
- In this fixture it is replaced with a pattern of 'x' characters
- Only the first and last 5 characters are preserved to show the format
- The middle section uses 'x' placeholders while maintaining the correct length

## File Overview

- `request.json`: Empty request payload (ARL is generated server-side)
- `response.json`: Response containing the anonymized ARL token

## Response Characteristics

The response contains:
- A single ARL token string in the `results` field
- Standard error array (empty when successful)
- The token follows Deezer's 192 character format

### Important Notes

- The ARL token is a critical authentication credential
- This fixture maintains the format while being obviously non-functional
- The anonymized token cannot be used for authentication

## Purpose

These fixtures document the structure of ARL token responses while using clearly artificial values. They are provided for development and testing purposes only and cannot be used for actual authentication or content access.
