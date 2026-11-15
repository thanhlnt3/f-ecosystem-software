# Personal data inventory and cross-border data transfers (draft)

> English translation of [the Vietnamese original](../../legal/data-inventory.md). If they differ, the Vietnamese version is authoritative.

> **Technical draft, not legal advice.** This document lists the personal data F-Eco processes, where it is stored, and the flows that may leave Vietnam. It serves as input for a lawyer preparing the dossier under Vietnamese law on personal data protection and cybersecurity (tasks.md 15.4; spec platform-support "Data storage"). The instruments below must be confirmed by a lawyer as still in force and correctly applicable before launch (QUESTIONS.md).

## 1. Instruments to cross-check

- The Law on Personal Data Protection and its implementing decrees: personal data processing impact assessment, cross-border data transfer impact assessment, breach notification.
- Nghị định 13/2023/NĐ-CP (Decree 13/2023 on personal data protection): to what extent it still applies now that the Law is in force.
- Luật An ninh mạng 2018 (Law on Cybersecurity 2018) and Nghị định 53/2022/NĐ-CP (Decree 53/2022 guiding the Law on Cybersecurity): requirements to store data in Vietnam for domestic enterprises.

## 2. Personal data F-Eco processes

| Category | Data | Where stored | Purpose | Retention |
|---|---|---|---|---|
| Account | email, name, language, time zone, phone number (optional), password (hashed), TOTP, passkey | Kratos (Postgres) | sign-in, contact | until the account is deleted + 30-day grace period |
| Sessions and devices | IP address, user agent, approximate location | Kratos, `audit_log` | security, new-device alerts | access logs 90 days |
| App consent | app, scope, time of consent | Hydra (Postgres) | single sign-on, `/v1/me` API | until revoked |
| Invoices | name, tax code, invoice address | Postgres (encrypted with AES-256-GCM) | electronic invoices | per accounting law |
| Orders | request description, attachments, messages | Postgres, file directory | custom app-building orders | per contract |
| Payments | transaction ID, amount (cards are not stored) | Postgres | reconciliation, refunds | per accounting law |
| Developers | individual/organization name, domain, members | Postgres | developer portal | until they leave the platform |
| Notifications | title, content, delivery preferences | Postgres | notification center, email | 1 year (proposed) |
| Moderation, administration | audit logs, reason for viewing a profile | `audit_log` | accountability | 2 years (proposed) |

## 3. Third parties and flows that may leave Vietnam

| Party | Data sent | Location | Notes |
|---|---|---|---|
| Email provider (SMTP) | email, notification content | **not yet chosen** | if the server is abroad: a data transfer dossier is needed and it must be stated in the privacy policy |
| ZaloPay, VNPAY, MoMo | order ID, amount | Vietnam | no card data is sent (the gateway's own payment page is used) |
| MISA meInvoice | name, tax code, invoice address | Vietnam | |
| AI provider for F-Builder | user's request description | **not yet chosen** (QUESTIONS.md) | may be abroad |
| Child apps (developers) | data per the scopes the user consented to, a separate `sub` for each app | per developer | the user consents to each scope; revocable |
| Server infrastructure | the entire database, backups | **not yet chosen** | a data center in Vietnam is proposed |

## 4. To do before launch

1. A lawyer confirms the applicable instruments and F-Eco's obligations: data controller or data processor, for each flow.
2. Choose the server infrastructure and the backup storage location, preferably domestic.
3. Choose the email and AI providers; if they are abroad, prepare a transfer impact assessment dossier and file it as required.
4. Prepare the personal data processing impact assessment dossier.
5. Update the privacy policy: state the flows in section 3.
6. Data breach notification procedure: link it to `docs/security/incident-response.md`.
