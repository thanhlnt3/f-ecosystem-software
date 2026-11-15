# Personal data breach incident response procedure

> English translation of [the Vietnamese original](../../security/incident-response.md). If they differ, the Vietnamese version is authoritative.

This document satisfies the "Data breach handling" requirement in `openspec/changes/add-f-ecosystem-platform/specs/platform-security/spec.md` (tasks.md 4.5).

> **Legal note:** the deadlines and notification templates below must be confirmed by a lawyer before launch (QUESTIONS.md). Based on current understanding:
> - Decree 13/2023/NĐ-CP (Nghị định 13/2023/NĐ-CP, the personal data protection decree) requires notifying the Ministry of Public Security's specialized personal data protection agency (Department A05) no later than 72 hours after the breach occurs.
> - The 2025 Personal Data Protection Law and its guiding documents may change the deadline or the notification template.
>
> Always apply the document currently in force.

## 1. Scope

Applies when there are signs that personal data stored or processed by F-Eco has been accessed, copied, modified, deleted or disclosed without authorization. This data includes: accounts (Kratos), consents and access logs (PostgreSQL), OAuth tokens (Hydra), data export files, email, invoice and order information (when present). The scope also covers incidents at providers to which F-Eco transfers data (email, payment gateways, AI).

Examples: a leaked database backup; leaked `FECO_DATA_KEY`, `FECO_PAIRWISE_SECRET`, or Kratos or Hydra secrets; a sub-app found collecting data contrary to its declaration; an employee accessing profiles without a reason.

## 2. Roles

| Role | Responsibilities |
|---|---|
| Incident commander | Coordinates, decides the severity level, keeps the timeline, is accountable for meeting the notification deadline |
| Engineering | Investigates, contains, remediates, collects evidence |
| Legal and data protection | Assesses notification obligations, drafts notices to state authorities |
| Communications and support | Notifies users, answers questions |

Each role has a primary person and a backup. The contact list is kept outside the code repository.

## 3. Severity levels

| Level | Criteria | Response time |
|---|---|---|
| Critical | Leaked encryption keys or system secrets; leaked data of many users; sensitive data (invoice information, tax codes) | Start handling immediately, 24/7 |
| High | Leaked data of some users; a sub-app violating its declaration | Within 4 hours |
| Medium | Suspicion not yet confirmed; unauthorized access blocked in time | Within 1 business day |

## 4. Procedure

### Step 1: Detection and recording (hour 0)

- Sources: system alerts, reports from users or developers, scan results, provider notifications.
- Open an incident file and record the **time of detection**. The notification deadline is counted from here.
- Designate an incident commander.

### Step 2: Scope assessment (within the first 24 hours)

Use the audit log (the `audit_log` table, append-only, retained for 1 year):

```sql
-- Activity of one account
SELECT at, action, target, ip, details FROM audit_log
WHERE actor_id = '<identity id>' ORDER BY at;

-- Failed logins and security actions in the suspect window
SELECT at, action, actor_id, ip FROM audit_log
WHERE at BETWEEN '<from>' AND '<to>'
  AND action IN ('auth.login_failed', 'auth.login', 'account.security_changed', 'security.not_me')
ORDER BY at;

-- Which apps have read a user's data (90 days)
SELECT client_id, scopes, created_at FROM data_access_log
WHERE subject = '<identity id>' ORDER BY created_at DESC;
```

Determine: the type of data affected, the number of users, the time window, the cause, and whether the data has been exfiltrated.

### Step 3: Containment

Available actions:

| Situation | Action |
|---|---|
| Account taken over | Kratos admin: `DELETE /admin/identities/{id}/sessions` (sign out of all devices); require the user to reset their password |
| Sub-app in violation | Hydra admin: `DELETE /admin/oauth2/auth/sessions/consent?subject=...&client=...` (revoke the app's consent and tokens); disable the client; emergency removal of the app (task group 9) |
| `FECO_DATA_KEY` leaked | Create a new key; re-encrypt webhook keys (developers are issued new keys) and delete old export files; change the key in the secret store |
| `FECO_PAIRWISE_SECRET` leaked | Assess the impact before changing it: changing the key changes the `sub` of every app |
| Kratos or Hydra secrets leaked (`secrets.cookie`, `secrets.cipher`, `secrets.system`) | Rotate keys following Ory's guidance (add the new key at the top of the list); sign out all sessions if needed |
| Payment gateway keys leaked | Revoke the keys at the gateway; change the keys; reconcile transactions |

Preserve evidence (logs, configuration snapshots) before changing the system.

### Step 4: Notification (within the legal deadline, currently understood as 72 hours)

- **State authority:** legal sends the notice using the prescribed template. The content includes: a description of the breach, the time, the type and quantity of data and data subjects, the measures taken, and a contact point. If information is incomplete, still send on time and supplement later.
- **Affected users:** send an email (template in section 6), and post a notice on the status page if the impact is wide.
- **Sub-app developers:** when their app is involved.

### Step 5: Remediation and recovery

Fix the root cause, verify there is no more unauthorized access, restore service, and monitor more closely for at least 30 days.

### Step 6: Lessons learned (within 2 weeks)

Hold a review meeting, without looking for someone to blame. Record the root cause, the timeline, what went well and what did not, and action items with owners. Update this document and the spec if needed.

## 5. Sample timeline

| Time | Action |
|---|---|
| Hour 0 | Detect, open the file, designate the commander |
| Hour 0 to 4 | Initial containment, preserve evidence |
| Hour 4 to 24 | Assess scope, decide on notification obligations |
| Before hour 72 | Send the notice to the state authority; notify affected users |
| Week 1 to 2 | Remediate, monitor, hold the lessons-learned meeting |

## 6. User email template

> **Subject:** Notice of a security incident involving your F-Eco account
>
> On [date], F-Eco discovered [a short, easy-to-understand description]. Data that may be affected: [list]. Your password [was / was not] stored in plain form (F-Eco only stores an Argon2id hash).
>
> We have [measures]. You should: reset your password, turn on two-factor authentication, and check the "Signed-in devices" and "Connected apps" pages.
>
> Contact: [contact point]. We apologize for this incident.

## 7. Drills

Run a drill at least once a year and before the public launch (tasks.md 4.5). Suggested scenarios:

1. **A PostgreSQL backup leaked** to a public storage location for 6 hours.
2. **The sub-app "SoundLab"** is found using user emails to send advertising, contrary to its declared purpose.
3. **The `FECO_DATA_KEY` key is committed by mistake** to a public code repository.

The drill report records: the date, participants, scenario, actual timeline compared with section 5, steps that could not be done or were slow, and improvements needed. Store the report with the security records, and do not put it in a public code repository if it contains internal information.
