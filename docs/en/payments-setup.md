# Configuring payment gateways, e-invoicing and email

> English translation of [the Vietnamese original](../payments-setup.md). If they differ, the Vietnamese version is authoritative.

This document is for F-Eco operators. It covers:
- registering sandbox accounts for ZaloPay, VNPAY, MoMo and MISA meInvoice;
- setting environment variables for `feco-server`;
- checking the configuration with `feco-catalog payments-check`;
- testing real callbacks through a tunnel;
- configuring the email sending domain (SPF, DKIM, DMARC).

The code already has adapters for all four services (tasks.md 13.2, 13.4, 14.3). What is still missing is the real keys and testing against real accounts.

> Never put keys in source code or commit them. Keys live only in environment variables or a secret store.
> Each gateway must have **all variables** in its table set (except those marked "optional"), or
> none at all. If any are missing, the server reports which variable is missing and does not start.
> Outside the `production` environment, `feco-server` accepts only the gateway's sandbox addresses: if
> one points at a real gateway, the server does not start.

## 1. Checking the configuration with one command

Set the environment variables as described in the sections below, then run:

```bash
cargo run -p feco-server --bin feco-catalog -- payments-check
```

The command sends each gateway a harmless query using a random order ID: it creates no order, charges no money and issues no invoice. Each line printed shows one of three results (the result strings are program output and stay in Vietnamese):

| Result | Meaning |
|---|---|
| `chưa cấu hình` (not configured) | The service's environment variables are missing |
| `đạt` (passed) | The gateway accepts the signature and partner code. For a random order ID it only answers "transaction not found". For MISA: a token was obtained. |
| `LỖI` (error) | The key, partner code or address is wrong, or the gateway blocks the IP address. The error line includes the gateway's code and message. |

The command exits with a non-zero code if any line shows `LỖI`, so it can be used in CI or during deployment.

## 2. ZaloPay

- **Registration**: a merchant account with ZaloPay. The sandbox has a public sample app in the `docs.zalopay.vn` documentation (app 2554 works; app 2553 returns error -402).
- **Environment variables**:

  | Variable | Value |
  |---|---|
  | `FECO_ZALOPAY_APP_ID` | app ID |
  | `FECO_ZALOPAY_KEY1` | signs outgoing requests |
  | `FECO_ZALOPAY_KEY2` | verifies callback signatures |
  | `FECO_ZALOPAY_ENDPOINT` | sandbox: `https://sb-openapi.zalopay.vn/` |

- **Callback**: `{FECO_PAYMENT_CALLBACK_BASE}v1/payments/zalopay/callback`. F-Eco sends this address with each order; it does not need to be registered in advance.
- **Real test**:
  ```bash
  FECO_ZALOPAY_SANDBOX_APP_ID=… FECO_ZALOPAY_SANDBOX_KEY1=… FECO_ZALOPAY_SANDBOX_KEY2=… \
    cargo test -p feco-server --test zalopay_sandbox
  ```
  Remaining: pay a real transaction on the sandbox page to test the callback and refund (section 6).

## 3. VNPAY

- **Sandbox registration**: at `https://sandbox.vnpayment.vn/devreg/`. VNPAY emails you the `TmnCode` and `HashSecret`.
- **Environment variables**:

  | Variable | Value |
  |---|---|
  | `FECO_VNPAY_TMN_CODE` | website code |
  | `FECO_VNPAY_HASH_SECRET` | HMAC-SHA512 signing key |
  | `FECO_VNPAY_PAYMENT_URL` | sandbox: `https://sandbox.vnpayment.vn/paymentv2/vpcpay.html` |
  | `FECO_VNPAY_API_URL` | querydr, refund (optional: derived from the payment URL) |

- **IPN**: register it **in the VNPAY merchant portal**; it is not sent per transaction: `{FECO_PAYMENT_CALLBACK_BASE}v1/payments/vnpay/ipn` (GET).
- **Check**: `payments-check` queries a transaction ID that does not exist.
  - `vnp_ResponseCode` 91 (transaction not found): passed.
  - 97 (invalid signature) or 02 (invalid TmnCode): error.
- **Needs real testing** (QUESTIONS.md 65): the signature algorithm for querydr and refund, and the production address.

## 4. MoMo

- **Registration**: the business registers at the MoMo Business portal (M4B) to receive the `partnerCode`, `accessKey` and `secretKey` for the test environment.
- **Environment variables**:

  | Variable | Value |
  |---|---|
  | `FECO_MOMO_PARTNER_CODE` | partner code |
  | `FECO_MOMO_ACCESS_KEY` | access key |
  | `FECO_MOMO_SECRET_KEY` | HMAC-SHA256 signing key |
  | `FECO_MOMO_ENDPOINT` | sandbox: `https://test-payment.momo.vn/v2/gateway/api/create` |

- **IPN**: `{FECO_PAYMENT_CALLBACK_BASE}v1/payments/momo/ipn` (POST; F-Eco returns 204). F-Eco sends this address with each order.
- **Check**: `payments-check` queries an order that does not exist.
  - `resultCode` 42 (order does not exist): passed.
  - 11 (invalid signature or access denied): error.
- **Real test**:
  ```bash
  FECO_MOMO_SANDBOX_PARTNER_CODE=… FECO_MOMO_SANDBOX_ACCESS_KEY=… FECO_MOMO_SANDBOX_SECRET_KEY=… \
    cargo test -p feco-server --test momo_sandbox
  ```
- **Note** (13/11/2026): from a development machine, `test-payment.momo.vn` returns `403 Forbidden` (an nginx error page) for every request, including the root page, even when using the public test keys in MoMo's documentation. The gateway may only be open to IP addresses registered with the M4B account. When this error occurs, `payments-check` and the adapter report `HTTP 403 Forbidden: phản hồi không phải JSON (…)` (the message text is program output: "response is not JSON").

## 5. MISA meInvoice (e-invoicing)

- **Registration**: the business's meInvoice account, plus an application at `developer.misa.vn` to obtain the `ClientID` and `ClientSecret`. You also need:
  - an invoice series registered with the tax authority;
  - an HSM digital signing certificate (F-Eco issues with `SignType` 2).
- **Environment variables**:

  | Variable | Value |
  |---|---|
  | `FECO_MISA_CLIENT_ID`, `FECO_MISA_CLIENT_SECRET` | application at MISA Developer |
  | `FECO_MISA_TAX_CODE` | F-Eco's tax code |
  | `FECO_MISA_USERNAME`, `FECO_MISA_PASSWORD` | meInvoice account |
  | `FECO_MISA_INV_SERIES` | invoice series, for example `1C26TFE` |
  | `FECO_MISA_VAT_RATE` | VAT rate, to be confirmed by accounting (QUESTIONS.md 62) |
  | `FECO_MISA_ENDPOINT` | the account's meInvoice API address |

  Without these variables, F-Eco still creates a **draft invoice** (HTML) attached to the email.
- **Check**: `payments-check` fetches a new token. No invoice is issued.
- **Needs real testing** (QUESTIONS.md 67): adjustment (reduction) invoices and the `OrgInvDate` format.

## 6. Testing real callbacks through a tunnel

The gateway must be able to reach the F-Eco server, so on a development machine you need to open a temporary tunnel:

```bash
~/.local/feco-devtools/cloudflared tunnel --url http://127.0.0.1:8080
# take the https://….trycloudflare.com address it prints, then run feco-server with
FECO_PAYMENT_CALLBACK_BASE=https://….trycloudflare.com/
```

Then:
1. Create a payment request for a test order (`feco-catalog payment-link <order ID>`), then open the gateway's payment page **in your own browser**. The development environment's built-in browser blocks some resources on the gateway pages.
2. Pay with the gateway's sandbox wallet or test card.
3. Check `/payments`: the transaction should change to "Thành công" (success) thanks to the callback, without waiting for an active query.
4. Make a partial refund with `feco-catalog refund <transaction-id> <amount> <reason>`, then run `feco-catalog payments-due` to query the refund result.

## 7. Email sending domain (SPF, DKIM, DMARC)

1. Choose an SMTP relay that signs DKIM: a transactional email provider, or Postfix with OpenDKIM.
2. Set `FECO_SMTP_URL=smtps://user:pass@smtp.example:465`. Production requires TLS.
3. Add the DNS records. Check with the command below, and rerun until every line reports "đạt" (passed):
   ```bash
   cargo run -p feco-server --bin feco-catalog -- mail-dns <domain> <DKIM selector>
   ```

## 8. Notification language

Emails and notifications in the launcher follow the language the user chose: the `locale` trait (`vi` or `en`), set at sign-up or in Settings.
- Both languages are fully available for: order, payment, refund and invoice notifications.
- Emails sent to developers (moderation, appeals, revocations, invitations) and account security emails contain both languages in the same message.
