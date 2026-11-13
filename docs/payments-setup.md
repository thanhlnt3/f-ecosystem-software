# Cấu hình cổng thanh toán, hóa đơn điện tử và email

Tài liệu cho người vận hành F-Eco. Nội dung gồm:
- đăng ký tài khoản thử (sandbox) của ZaloPay, VNPAY, MoMo, MISA meInvoice;
- đặt biến môi trường cho `feco-server`;
- kiểm cấu hình bằng `feco-catalog payments-check`;
- thử callback thật qua tunnel;
- cấu hình tên miền gửi email (SPF, DKIM, DMARC).

Mã đã có sẵn adapter cho cả bốn dịch vụ (tasks.md 13.2, 13.4, 14.3). Chỉ còn thiếu khóa thật và việc thử với tài khoản thật.

> Không đưa khóa vào mã nguồn hay commit. Khóa chỉ nằm trong biến môi trường hoặc kho bí mật.
> Mỗi cổng phải đặt **đủ mọi biến** trong bảng của cổng đó (trừ biến ghi "không bắt buộc"), hoặc
> không đặt biến nào. Đặt thiếu thì máy chủ báo thiếu biến nào và không khởi động.
> Ngoài môi trường `production`, `feco-server` chỉ nhận địa chỉ sandbox của cổng: trỏ nhầm sang
> cổng thật thì máy chủ không khởi động.

## 1. Kiểm cấu hình bằng một lệnh

Đặt biến môi trường như các mục dưới, rồi chạy:

```bash
cargo run -p feco-server --bin feco-catalog -- payments-check
```

Lệnh gửi tới mỗi cổng một truy vấn vô hại bằng mã đơn ngẫu nhiên: không tạo đơn, không trừ tiền, không phát hành hóa đơn. Mỗi dòng in ra một trong ba kết quả:

| Kết quả | Nghĩa |
|---|---|
| `chưa cấu hình` | Thiếu biến môi trường của dịch vụ đó |
| `đạt` | Cổng chấp nhận chữ ký và mã đối tác. Nó chỉ trả lời "không tìm thấy giao dịch" cho mã đơn ngẫu nhiên. Với MISA: lấy được token. |
| `LỖI` | Khóa, mã đối tác hoặc địa chỉ sai, hoặc cổng chặn địa chỉ IP. Dòng lỗi kèm mã và thông báo của cổng. |

Lệnh trả mã thoát khác 0 nếu có dòng `LỖI`, nên dùng được trong CI hoặc khi triển khai.

## 2. ZaloPay

- **Đăng ký**: tài khoản merchant ở ZaloPay. Sandbox có app mẫu công khai trong tài liệu `docs.zalopay.vn` (app 2554 dùng được; app 2553 báo lỗi -402).
- **Biến môi trường**:

  | Biến | Giá trị |
  |---|---|
  | `FECO_ZALOPAY_APP_ID` | mã app |
  | `FECO_ZALOPAY_KEY1` | ký yêu cầu gửi đi |
  | `FECO_ZALOPAY_KEY2` | kiểm chữ ký callback |
  | `FECO_ZALOPAY_ENDPOINT` | sandbox: `https://sb-openapi.zalopay.vn/` |

- **Callback**: `{FECO_PAYMENT_CALLBACK_BASE}v1/payments/zalopay/callback`. F-Eco gửi địa chỉ này cùng từng đơn, không phải khai trước.
- **Thử thật**:
  ```bash
  FECO_ZALOPAY_SANDBOX_APP_ID=… FECO_ZALOPAY_SANDBOX_KEY1=… FECO_ZALOPAY_SANDBOX_KEY2=… \
    cargo test -p feco-server --test zalopay_sandbox
  ```
  Còn lại: trả một giao dịch thật trên trang sandbox để thử callback và hoàn tiền (mục 6).

## 3. VNPAY

- **Đăng ký sandbox**: tại `https://sandbox.vnpayment.vn/devreg/`. VNPAY gửi `TmnCode` và `HashSecret` qua email.
- **Biến môi trường**:

  | Biến | Giá trị |
  |---|---|
  | `FECO_VNPAY_TMN_CODE` | mã website |
  | `FECO_VNPAY_HASH_SECRET` | khóa ký HMAC-SHA512 |
  | `FECO_VNPAY_PAYMENT_URL` | sandbox: `https://sandbox.vnpayment.vn/paymentv2/vpcpay.html` |
  | `FECO_VNPAY_API_URL` | querydr, refund (không bắt buộc: suy ra từ địa chỉ thanh toán) |

- **IPN**: khai **trong cổng merchant của VNPAY**, không gửi theo từng giao dịch: `{FECO_PAYMENT_CALLBACK_BASE}v1/payments/vnpay/ipn` (GET).
- **Kiểm**: `payments-check` truy vấn một mã giao dịch không tồn tại.
  - `vnp_ResponseCode` 91 (không tìm thấy giao dịch): đạt.
  - 97 (sai chữ ký) hoặc 02 (sai TmnCode): lỗi.
- **Cần thử thật** (QUESTIONS.md 65): thuật toán chữ ký của querydr và refund, địa chỉ production.

## 4. MoMo

- **Đăng ký**: doanh nghiệp đăng ký tại cổng MoMo Business (M4B) để nhận `partnerCode`, `accessKey`, `secretKey` của môi trường thử.
- **Biến môi trường**:

  | Biến | Giá trị |
  |---|---|
  | `FECO_MOMO_PARTNER_CODE` | mã đối tác |
  | `FECO_MOMO_ACCESS_KEY` | khóa truy cập |
  | `FECO_MOMO_SECRET_KEY` | khóa ký HMAC-SHA256 |
  | `FECO_MOMO_ENDPOINT` | sandbox: `https://test-payment.momo.vn/v2/gateway/api/create` |

- **IPN**: `{FECO_PAYMENT_CALLBACK_BASE}v1/payments/momo/ipn` (POST; F-Eco trả 204). F-Eco gửi địa chỉ này cùng từng đơn.
- **Kiểm**: `payments-check` truy vấn một đơn không tồn tại.
  - `resultCode` 42 (đơn không tồn tại): đạt.
  - 11 (sai chữ ký hoặc truy cập bị từ chối): lỗi.
- **Thử thật**:
  ```bash
  FECO_MOMO_SANDBOX_PARTNER_CODE=… FECO_MOMO_SANDBOX_ACCESS_KEY=… FECO_MOMO_SANDBOX_SECRET_KEY=… \
    cargo test -p feco-server --test momo_sandbox
  ```
- **Lưu ý** (13/11/2026): từ máy phát triển, `test-payment.momo.vn` trả `403 Forbidden` (trang lỗi của nginx) cho mọi yêu cầu, kể cả trang gốc, và cả khi dùng khóa thử công khai trong tài liệu của MoMo. Có thể cổng chỉ mở cho địa chỉ IP đã đăng ký với tài khoản M4B. Khi gặp lỗi này, `payments-check` và adapter báo `HTTP 403 Forbidden: phản hồi không phải JSON (…)`.

## 5. MISA meInvoice (hóa đơn điện tử)

- **Đăng ký**: tài khoản meInvoice của doanh nghiệp, cùng ứng dụng ở `developer.misa.vn` để lấy `ClientID` và `ClientSecret`. Cần thêm:
  - ký hiệu hóa đơn đã đăng ký với cơ quan thuế;
  - chứng thư ký số HSM (F-Eco phát hành với `SignType` 2).
- **Biến môi trường**:

  | Biến | Giá trị |
  |---|---|
  | `FECO_MISA_CLIENT_ID`, `FECO_MISA_CLIENT_SECRET` | ứng dụng ở MISA Developer |
  | `FECO_MISA_TAX_CODE` | mã số thuế của F-Eco |
  | `FECO_MISA_USERNAME`, `FECO_MISA_PASSWORD` | tài khoản meInvoice |
  | `FECO_MISA_INV_SERIES` | ký hiệu hóa đơn, ví dụ `1C26TFE` |
  | `FECO_MISA_VAT_RATE` | thuế suất, kế toán xác nhận (QUESTIONS.md 62) |
  | `FECO_MISA_ENDPOINT` | địa chỉ API meInvoice của tài khoản |

  Thiếu các biến này thì F-Eco vẫn lập **hóa đơn nháp** (HTML) gửi kèm email.
- **Kiểm**: `payments-check` lấy token mới. Không phát hành hóa đơn nào.
- **Cần thử thật** (QUESTIONS.md 67): hóa đơn điều chỉnh giảm, định dạng `OrgInvDate`.

## 6. Thử callback thật qua tunnel

Cổng cần gọi được về máy chủ F-Eco, nên ở máy phát triển phải mở một tunnel tạm:

```bash
~/.local/feco-devtools/cloudflared tunnel --url http://127.0.0.1:8080
# lấy địa chỉ https://….trycloudflare.com in ra, rồi chạy feco-server với
FECO_PAYMENT_CALLBACK_BASE=https://….trycloudflare.com/
```

Sau đó:
1. Tạo yêu cầu thanh toán cho một đơn thử (`feco-catalog payment-link <mã đơn>`), rồi mở trang thanh toán của cổng **trên trình duyệt của bạn**. Trình duyệt tích hợp của môi trường phát triển chặn một số tài nguyên của trang cổng.
2. Trả bằng ví sandbox hoặc thẻ thử của cổng.
3. Xem `/payments`: giao dịch phải chuyển "Thành công" nhờ callback, không cần đợi truy vấn chủ động.
4. Hoàn một phần bằng `feco-catalog refund <transaction-id> <số tiền> <lý do>`, rồi chạy `feco-catalog payments-due` để truy vấn kết quả hoàn.

## 7. Tên miền gửi email (SPF, DKIM, DMARC)

1. Chọn máy chủ chuyển tiếp SMTP có ký DKIM: nhà cung cấp email giao dịch, hoặc Postfix kèm OpenDKIM.
2. Đặt `FECO_SMTP_URL=smtps://user:pass@smtp.example:465`. Production bắt buộc TLS.
3. Thêm bản ghi DNS. Kiểm bằng lệnh sau, chạy lại cho tới khi mọi dòng báo "đạt":
   ```bash
   cargo run -p feco-server --bin feco-catalog -- mail-dns <tên miền> <selector DKIM>
   ```

## 8. Ngôn ngữ của thông báo

Email và thông báo trong launcher theo ngôn ngữ người dùng chọn: trait `locale` (`vi` hoặc `en`), đặt khi đăng ký hoặc trong Cài đặt.
- Có đủ hai ngôn ngữ: thông báo đơn hàng, thanh toán, hoàn tiền, hóa đơn.
- Email gửi nhà phát triển (kiểm duyệt, kháng nghị, thu hồi, lời mời) và email bảo mật của tài khoản có cả hai ngôn ngữ trong cùng một thư.
