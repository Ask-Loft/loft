# Auth Specification

## Purpose

How a customer signs in to the shop and how long a signed-in session lasts.

## Requirements

### Requirement: Sign in with email
The shop SHALL sign a customer in with an email address and a password.

#### Scenario: Correct password
- **WHEN** a customer gives a registered email address and its password
- **THEN** the shop SHALL start a session for that customer

#### Scenario: Wrong password
- **WHEN** a customer gives a registered email address and another password
- **THEN** the shop SHALL refuse the sign-in
- **AND** no session is started

### Requirement: Session lifetime
The shop MUST end a session after 30 minutes without a request.

#### Scenario: Idle session
- **WHEN** 30 minutes pass without a request in a session
- **THEN** the shop SHALL end the session
