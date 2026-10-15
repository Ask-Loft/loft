# Invoices Specification

## Purpose

How the shop issues an invoice for each paid order and keeps it for the customer.

## Requirements

### Requirement: Invoice per order
The shop SHALL issue one invoice for each paid order.

#### Scenario: Order paid
- **WHEN** an order is paid
- **THEN** the shop SHALL issue an invoice that names the order
