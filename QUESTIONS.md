# Câu hỏi còn mở

Ghi lại những điểm spec chưa rõ hoặc cần chủ dự án quyết định (theo AGENTS.md).

## Nhóm 2: tài khoản

1. **Nhà cung cấp thử thách chống bot cho production.** Hiện chỉ có thử thách thử nghiệm
   (ô đánh dấu) cho dev và CI; production bị chặn khởi động cho tới khi chọn nhà cung cấp
   thật. Ứng viên: Cloudflare Turnstile, hCaptcha. Cần chọn và cân nhắc việc gửi dữ liệu
   ra nước ngoài (spec platform-support, "Lưu trữ dữ liệu").
2. **Khôi phục mật khẩu dùng mã, không dùng liên kết.** Spec ghi "liên kết email dùng một
   lần, có hạn 30 phút". Kratos chỉ cho một thời hạn chung cho mọi liên kết (xác minh email
   cần 24 giờ), nên khôi phục dùng **mã 30 phút** gửi qua email. Mức an toàn tương đương.
   Cần xác nhận, hoặc sửa spec cho khớp.
3. **Bộ đếm đăng nhập sai nằm trong bộ nhớ.** Đúng khi chạy một bản feco-server. Khi chạy
   nhiều bản cần chuyển sang Valkey (đã có trong hạ tầng dev).
4. **Đăng nhập Google và GitHub (task 2.3).** Cần chủ dự án tạo ứng dụng OAuth trên Google
   Cloud Console và GitHub (Settings → Developer settings → OAuth Apps), với địa chỉ chuyển
   hướng `http://127.0.0.1:4433/self-service/methods/oidc/callback/google` (và `.../github`)
   cho dev, rồi cung cấp client ID và secret qua biến môi trường. Không đưa secret vào mã nguồn.
5. **Passkey khi phát triển.** WebAuthn không chạy trên địa chỉ IP, nên muốn thử passkey ở máy
   cần chuyển dev sang tên miền `localhost` (Kratos, feco-server, cookie). Có làm ngay không?
6. **Kratos ở staging và production dùng PostgreSQL.** Dev và CI dùng SQLite trong bộ nhớ, vốn
   báo "database table is locked" khi ghi đồng thời (test E2E vì vậy chạy lần lượt).

## Nhóm 3: dữ liệu người dùng và đồng ý

7. **Chính sách quyền riêng tư và điều khoản sử dụng (task 3.6)** cần luật sư soạn và rà soát
   theo pháp luật Việt Nam về bảo vệ dữ liệu cá nhân. Mã nguồn chưa có văn bản này.
8. **Đăng ký webhook và scope của app** hiện được tạo trực tiếp qua API quản trị Hydra và bảng
   `app_webhooks` (dùng trong test). Cổng nhà phát triển (nhóm 8) sẽ ghi các thông tin này từ
   `f-app.toml`.
9. **File xuất dữ liệu lưu trong PostgreSQL** (cột `bytea`). Khi dữ liệu lớn (dự án F-Builder),
   nên chuyển sang kho đối tượng (design D11).

## Nhóm 4: bảo mật

10. **Bộ đếm giới hạn tần suất nằm trong bộ nhớ** (giống bộ đếm đăng nhập sai). Khi chạy nhiều bản
    feco-server cần chuyển sang Valkey. Ngưỡng mặc định: 1200 yêu cầu/phút mỗi IP, 1200 lần/phút
    mỗi app và 120 lần/phút mỗi người dùng ở `/v1/me`. Cần chốt ngưỡng cho production.
11. **Vai trò cơ sở dữ liệu ở production**: feco-server phải kết nối bằng vai trò không được
    `ALTER TABLE`/`DISABLE TRIGGER`, nếu không trigger bảo vệ nhật ký kiểm toán có thể bị tắt.
12. **Thời hạn thông báo sự cố** trong `docs/security/incident-response.md` (hiện ghi 72 giờ theo
    Nghị định 13/2023/NĐ-CP) cần luật sư xác nhận theo văn bản đang có hiệu lực.
13. **TLS**: feco-server chạy sau proxy TLS (ví dụ của nhà cung cấp đám mây). Quét cấu hình TLS 1.2+
    làm khi có staging.

## Nhóm 5: phân phối và chuỗi tin cậy

14. **Người giữ khóa root**: cần chỉ định 3 người (khác bộ phận, không cùng đi công tác) và thiết bị
    giữ khóa (USB HSM hay máy ngoại tuyến mã hóa đĩa) trước buổi ký root thật và buổi diễn tập 5.3.
15. **KMS cho khóa targets, snapshot, timestamp**: phụ thuộc nhà cung cấp hạ tầng (task 1.4).
    `tough` có sẵn nguồn khóa cho AWS KMS; nếu chọn nhà cung cấp trong nước không có KMS tương thích
    thì cần HSM riêng.
16. **Thời hạn timestamp 1 giờ** (để thu hồi tới launcher trong 1 giờ kể cả khi bị đóng băng metadata)
    buộc máy chủ ký lại timestamp liên tục và launcher có đồng hồ đúng. Nếu máy người dùng lệch giờ
    hơn khoảng đó, launcher sẽ báo metadata hết hạn. Cần quyết định có nới lên (ví dụ 4 giờ) và
    chấp nhận cửa sổ đóng băng dài hơn không.

## Nhóm 6: F-Launcher

17. **Chứng thư ký mã cho launcher tự cập nhật**: spec yêu cầu bản cập nhật launcher ký Authenticode
    (Windows) và Developer ID + notarize (macOS). Cần mua chứng thư (EV code signing) và tài khoản
    Apple Developer trước khi làm task 6.6 (tự cập nhật, bộ cài ký mã, xác minh trên máy ảo sạch).
18. **Kiểm tra trên Windows 11 và macOS 13** (task 6.1): máy dev hiện là Windows 10 và chưa có máy Mac.
    Cần máy (hoặc runner CI có giao diện) để kiểm tra tay giao diện, Keychain và trình duyệt mặc định.
19. **Mở app khi người dùng chưa từng đồng ý**: mã khởi chạy không hiện được màn hình đồng ý nên trả
    `consent_required`, và app phải tự mở luồng đăng nhập qua trình duyệt một lần. Có muốn launcher
    tự mở màn hình đồng ý trong trình duyệt trước khi chạy app lần đầu không?
20. **Giới hạn kích thước gói**: hiện chỉ có độ dài đã ký trong TUF. Cần chốt dung lượng tối đa của
    một gói và tổng dung lượng sau giải nén (chống "zip bomb") khi làm kiểm duyệt (nhóm 9).
21. **Nguồn danh mục trong launcher**: trước khi có dịch vụ danh mục (nhóm 7), launcher lấy tên app,
    phiên bản và file chạy từ trường `custom` đã ký trong metadata TUF. Khi có dịch vụ danh mục, mô tả,
    ảnh và đánh giá lấy từ dịch vụ, còn thông tin cài đặt vẫn lấy từ TUF.

## Nhóm 7: danh mục F-Store

22. **"Đã cài" để được đánh giá** do launcher báo (`POST /v1/catalog/installs` bằng token của
    launcher), tức là người dùng tự khai. Có cần chặt hơn (ví dụ chỉ tính khi đã tải gói từ máy chủ
    phân phối, hoặc đã mở app ít nhất một lần) không?
23. **Danh sách danh mục** (Giáo dục, Công cụ, Âm nhạc...) hiện là chữ tự do do nhà phát triển ghi.
    Cần chốt danh sách cố định và tên tiếng Anh tương ứng trước khi làm cổng nhà phát triển (nhóm 8).
24. **Quyền quản trị**: hàng chờ báo cáo và việc gỡ app tạm dùng công cụ dòng lệnh `feco-catalog` trên
    máy chủ. Bảng quản trị có phân quyền (nhóm 14) cần biết ai là quản trị viên và có cần hai người
    duyệt cho việc gỡ app không.
25. **Một nhận xét cho mỗi người mỗi app** (viết lại thì cập nhật). Có cần giữ lịch sử các lần sửa,
    và nhận xét có gắn với phiên bản app đang dùng không?

## Nhóm 8: cổng nhà phát triển

26. **Nội dung Thỏa thuận nhà phát triển**: cổng ghi phiên bản (`2026-11-01`) và thời điểm chấp
    nhận, nhưng văn bản thỏa thuận chưa có. Cần luật sư soạn (cùng điều khoản IP, chia sẻ doanh
    thu, chính sách nội dung) trước khi mở đăng ký thật.
27. **Mỗi người dùng thuộc tối đa một tài khoản nhà phát triển**. Có cần cho một người vừa có tài
    khoản cá nhân vừa là thành viên của tổ chức không?
28. **Xác minh tên miền không kiểm DNSSEC** và hỏi máy chủ DNS công cộng (`FECO_DNS_SERVERS`). Đủ cho
    dấu "Đã xác minh", hay cần thêm xác minh pháp nhân (đối chiếu mã số doanh nghiệp)?
29. **Passkey làm lớp xác thực thứ hai của nhà phát triển**: hiện chỉ TOTP được tính (task 2.4 cũng
    còn thiếu test passkey). Cần phân biệt được passkey đã đăng ký thật trong Kratos.
30. **Xác minh mật mã chữ ký mã**: bước nộp chỉ kiểm tệp chạy có mang chữ ký. Khâu kiểm duyệt
    (task 9.1) cần máy Windows (`signtool verify`) và macOS (`codesign`, `spctl`, `stapler`) để
    xác minh chữ ký, chuỗi chứng thư và notarize. Cần quyết định hạ tầng cho hai máy quét này.
31. **App trả phí**: manifest chưa có giá; mọi app phát hành qua cổng hiện miễn phí. Giá và chia sẻ
    doanh thu thuộc nhóm thanh toán (12–13).
32. **Kho gói**: gói tải lên nằm trên đĩa của máy chủ (`FECO_PACKAGE_DIR`). Production cần lưu trữ
    đối tượng (S3 tương thích) và phân phối `targets/` của TUF qua CDN; phụ thuộc task 1.4.
33. **Phát hành dần dựa trên mã thiết bị ngẫu nhiên của launcher** (không gắn tài khoản). Người dùng
    có nhiều máy có thể nhận bản khác nhau trên mỗi máy. Có chấp nhận không, hay chia nhóm theo
    tài khoản?

## Nhóm 9: kiểm duyệt

34. **Dịch vụ quét thương mại** (design D6): cần chọn nhà cung cấp có hợp đồng cho phép dùng thương
    mại. Hiện hai engine là engine chữ ký của F-Eco (EICAR và cơ sở dữ liệu SHA-256) và ClamAV; engine
    chữ ký chỉ bắt được mẫu đã biết, nên production cần thêm dịch vụ thương mại.
35. **ClamAV là GPL-2.0**: chạy thành dịch vụ riêng, không phân phối cho người dùng, nhưng phải ghi
    vào `third_party/license-exceptions.toml` (kèm người duyệt và ngày) khi đưa ảnh container
    ClamAV vào hạ tầng. Ai là người duyệt ngoại lệ này?
36. **Máy ảo dùng một lần** cho job quét và cho người duyệt chạy thử app: phụ thuộc nhà cung cấp hạ
    tầng (task 1.4). Hiện job chạy phân tích tĩnh trong tiến trình worker, không chạy tệp nào.
37. **Tạm khóa khi phát hiện mã độc** chặn tạo app và nộp phiên bản, nhưng các bản đang phát hành của
    tài khoản vẫn còn. Có thu hồi luôn mọi bản đang phát hành của tài khoản đó không, hay chờ điều tra?
38. **Mục tiêu 3 ngày làm việc cho 90% lượt duyệt thủ công**: cần lịch ngày làm việc (ngày lễ Việt
    Nam) và bảng quản trị có số liệu thời gian duyệt (nhóm 14) để đo và cảnh báo.
39. **Người duyệt** hiện ghi tên qua biến `FECO_REVIEWER` của công cụ dòng lệnh. Bảng quản trị cần
    vai trò người duyệt (có quyền duyệt nhưng không quản trị toàn hệ thống) và có cần hai người cho
    thao tác gỡ khẩn cấp không (đã hỏi ở câu 24).

## Nhóm 10: F-SDK

40. **Binding Python dùng `ctypes`, không dùng PyO3** như design D7: `ctypes` gọi thẳng thư viện C ABI
    (một thư viện dùng cho mọi phiên bản Python, không cần biên dịch lại cho từng bản, và chính C ABI
    cũng được kiểm qua đó). Đổi lại, gói Python phải kèm thư viện `fsdk_c` cho từng nền tảng. Có chấp
    nhận thay đổi này không? Tương tự, header `fsdk.h` đang viết tay thay vì sinh bằng `cbindgen`;
    có cần CI sinh lại và so sánh header không?
41. **Sandbox cho app mẫu** (task 10.2): app mẫu đang được kiểm trên hạ tầng dev cô lập của lần chạy
    test. Môi trường sandbox dành cho nhà phát triển (task 8.6) chưa có; khi có, F-SDK cần một cấu
    hình trỏ tới sandbox (địa chỉ issuer và máy chủ riêng).
42. **Quyền sử dụng** hiện được cấp bằng tay (`feco-catalog grant`). Nhóm 12–13 sẽ cấp tự động khi bàn
    giao đơn xây app hay thanh toán. Cần chốt: một người có thể có nhiều quyền cùng loại từ nhiều
    nguồn không (hiện mỗi loại một bản ghi), và gói thuê bao hết hạn thì có gửi
    `entitlement.changed` không (hiện chỉ gửi khi cấp hoặc thu hồi).
43. **Gói TypeScript** chạy thẳng tệp `.ts` trên Node.js ≥ 22.6 và chỉ có luồng đăng nhập qua máy chủ
    loopback (app desktop). Design D7 muốn chạy được cả trong trình duyệt: cần luồng chuyển hướng
    trong trình duyệt và nơi giữ refresh token an toàn. Khi phát hành lên npm cần dựng sang
    JavaScript, và cần xác nhận ai giữ tên `@f-eco` trên npm.
44. **Báo lỗi khi app gặp sự cố** chỉ gửi phiên bản (đếm số lần), không gửi stack trace hay thông tin
    máy, để không thu thập dữ liệu cá nhân. Nhà phát triển có cần thêm thông tin (ví dụ hệ điều
    hành) không, và có cần người dùng đồng ý riêng không?
45. **Ngừng hỗ trợ phiên bản API**: chính sách giữ ít nhất 12 tháng đã ghi trong tài liệu, nhưng chưa
    có phiên bản nào bị ngừng nên header `Deprecation`/`Sunset` chưa được gắn. Ai quyết định và
    thông báo ngừng hỗ trợ (email cho nhà phát triển có app đang gọi phiên bản đó)?

## Nhóm 11: F-Builder

46. **F-Builder là Cargo workspace riêng** trong `clients/f-builder` (repo vẫn là một, nhưng có hai
    `Cargo.lock`): giữ nguyên edition 2024, `[patch]` wgpu-hal, profile và `xtask` của bản gốc để còn
    lấy bản sửa từ Craft Anything (`git subtree pull`). Có chấp nhận hai lockfile không?
47. **Định danh ứng dụng** `io.github.thanhlnt3.f-builder` (bundle id macOS, mã Windows, thư mục dữ
    liệu) dựa trên tài khoản GitHub vì F-Eco chưa có tên miền. Khi có tên miền chính thức, đổi trước
    lần phát hành đầu (đổi sau sẽ mất cài đặt cũ của người dùng).
48. **Cửa sổ DMG trên macOS**: `packaging/macos/dmg/DS_Store` của bản gốc mang tên ổ đĩa cũ nên đã
    gỡ; cần dựng lại bằng `generate.py` (cần resvg, `ds_store`, `mac_alias`) khi làm đóng gói macOS.
49. **"Chạy mã do agent tạo"** (spec app-builder, "Chạy lệnh có kiểm soát"): F-Builder hỏi trước khi
    chạy app (`cargo run`, `node app.js`, `python main.py`…), nhưng vẫn cho chạy bản dựng và test, và
    lần chạy app để chụp màn hình hay tự kiểm tra (`--screenshot`, `--self-test`) vì đó là bước Kiểm
    tra của chính F-Builder. Cách hiểu này có đúng ý spec không?
50. **Một xác nhận cho hai việc**: xác nhận "phần mềm của tôi / tôi có quyền phân tích" khi đọc sâu
    cũng cho phép dùng tên sản phẩm tham khảo trong app mới. Có cần tách thành hai xác nhận không?
51. **Nộp từ F-Builder** dùng token nhà phát triển (tạo ở cổng, lưu trong kho bảo mật) thay vì đăng nhập
    F-Eco bằng F-SDK như design D8 nói, vì API nộp của cổng dùng token nhà phát triển. Có cần cổng nhận
    access token OAuth (scope nhà phát triển) để F-Builder đăng nhập một lần không?
52. **Gói nộp** hiện chỉ có tệp chạy chính của app (đủ cho app Rust một tệp). App có tài nguyên, thư
    viện kèm (Electron, Godot, Python) cần quy ước thư mục đóng gói trong `f-app.toml` hoặc `dist/`.
53. **Đơn "Nhờ F-Eco xây hộ"** được tạo nháp trong thư mục dự án; gửi đơn chờ API đơn hàng của nhóm 12
    (đính kèm spec, mã nguồn, giới hạn kích thước).

## Nhóm 12: Đơn xây app theo yêu cầu

54. **Việc của đội F-Eco** (báo giá, bàn giao, trả lời, giải quyết tranh chấp, đánh dấu thanh toán)
    tạm làm bằng `feco-catalog order-*` cho tới khi có bảng quản trị (nhóm 14). Kết quả thanh toán
    (`order-paid`) sẽ do cổng thanh toán của nhóm 13 gọi. Gửi đơn từ F-Builder (task 11.6) cần một
    API đơn hàng dùng token (hiện chỉ có biểu mẫu web với phiên đăng nhập); cần chốt cách xác thực.
55. **App riêng tư chỉ ẩn, chưa khóa**: F-Store và F-Launcher chỉ hiện bản build bàn giao cho khách
    hàng của đơn, nhưng metadata TUF và gói trên máy chủ phân phối vẫn công khai (ai biết tên target
    thì tải được). Nếu mã nguồn hay bản build cần bí mật trước khi trả đủ, cần tải gói có xác thực
    hoặc mã hóa gói theo khách hàng. Cần quyết định. Ngoài ra app phải được đặt riêng tư
    (`feco-catalog private`) trước khi phát hành phiên bản đầu, để cờ riêng tư có trong chữ ký TUF.
56. **Ngày làm việc** (hạn báo giá 3 ngày, phản hồi tranh chấp 2 ngày) hiện chỉ bỏ thứ Bảy và Chủ
    nhật, chưa trừ ngày lễ Việt Nam (như câu 38). Hạn chỉ được nhắc trong email, chưa có cảnh báo khi
    đội F-Eco trễ hạn.
57. **Một số quy tắc chưa rõ trong spec, đang làm như sau**: báo giá cần ít nhất hai mốc (mốc đầu là
    đặt cọc ≤ 30%); hủy sau đặt cọc giữ tiền đặt cọc khi `started_at` đã có, kể cả khi mốc 1 chưa
    bàn giao; hoàn một phần khi tranh chấp phải lớn hơn 0 và nhỏ hơn giá mốc; tệp kèm mỗi tin nhắn
    tối đa 50 MB; yêu cầu sửa không cần đính kèm. Đơn tự nghiệm thu mốc cuối sau 14 ngày cũng
    chuyển "Hoàn tất" như khi khách nghiệm thu.
58. **Điều khoản dịch vụ xây app** (`/policy/custom-build`) là bản nháp do tôi viết theo spec, cần
    luật sư rà soát (task 12.6): đặc biệt điều khoản giữ tiền đặt cọc, quyền sở hữu mã nguồn mặc
    định, thuế trong giá, và việc tự nghiệm thu sau 14 ngày. Đổi nội dung thì tăng `TOS_VERSION`;
    đơn cũ giữ phiên bản đã chấp nhận.

## Nhóm 13: Thanh toán và hóa đơn

59. **ZaloPay là cổng đầu tiên** (chủ dự án chọn ngày 11/11/2026), thay VNPAY và MoMo trong design
    D9; hai cổng này thêm sau qua adapter nếu cần. Bộ khóa sandbox mẫu `app_id` 2553 trong tài liệu
    ZaloPay đã cũ (cổng báo "Chữ ký không hợp lệ"); bộ mẫu 2554 dùng được. F-Eco nên có tài khoản
    sandbox riêng (đăng ký qua nhóm hỗ trợ của ZaloPay bằng số điện thoại và email) và khóa
    production khi ký hợp đồng. Khóa chỉ đặt qua biến môi trường, không có trong mã nguồn.
60. **Callback cần địa chỉ công khai**: ZaloPay không gọi được `127.0.0.1`, nên ở máy dev giao dịch
    được cập nhật nhờ truy vấn chủ động (mỗi phút, và khi khách quay về trang chuyển hướng). Cần
    staging có HTTPS công khai để thử callback thật, và một ví sandbox ZaloPay để thử hoàn tiền trên
    giao dịch đã trả thật. Mã phương thức `preferred_payment_method` (`vietqr`, `domestic_card`,
    `zalopay_wallet`, `international_card`) mới được sandbox chấp nhận khi tạo đơn, chưa kiểm
    trang cổng hiện đúng phương thức.
61. **Đối soát**: ZaloPay không công bố API báo cáo giao dịch, nên đối soát hằng ngày truy vấn từng
    giao dịch F-Eco đã tạo trong ngày. Cách này không phát hiện giao dịch có trên cổng mà F-Eco
    không hề tạo. Nếu hợp đồng có file đối soát (SFTP hoặc cổng merchant), cần thêm bước đọc file.
62. **Hóa đơn điện tử**: hiện là bản nháp nội bộ (HTML, ghi rõ "BẢN NHÁP"). Cần chọn nhà cung cấp
    (VNPT Invoice, Viettel S-Invoice, MISA meInvoice...), thông tin pháp lý của người bán (tên, mã
    số thuế, địa chỉ), thuế suất GTGT của dịch vụ xây phần mềm (có thể thuộc diện không chịu thuế,
    cần kế toán xác nhận), và quy trình ký số. Hóa đơn hiện ghi "đã gồm thuế" theo spec.
63. **Người phê duyệt hoàn tiền** tạm xác định bằng `FECO_REVIEWER` của `feco-catalog`, chưa chống
    giả mạo; bảng quản trị có passkey (nhóm 14) sẽ thay. Quy tắc đang dùng: lệnh của quản trị viên
    dưới 10.000.000 ₫ chạy ngay; từ 10.000.000 ₫ cần thêm một người khác; lệnh do hệ thống tạo khi
    khách hủy đơn cần một (hoặc hai) quản trị viên duyệt; hoàn tự động khi thu hai lần không cần
    duyệt, kể cả từ 10.000.000 ₫, và quản trị viên được báo. Cần xác nhận.
64. **4 số cuối thẻ**: callback của ZaloPay không có thông tin thẻ, nên `card_last4` để trống. Phí
    giao dịch, phí hoàn (`refund_fee_amount`) chưa xử lý: F-Eco hoàn đúng số tiền yêu cầu.
    `feco-catalog order-paid` (ghi nhận tay) chỉ còn dùng được ngoài production.
65. **VNPAY và MoMo** (chủ dự án chọn thêm cả hai, 12/11/2026) đã có adapter theo tài liệu chính
    thức, mới thử với cổng giả. Cần khóa sandbox: VNPAY đăng ký ở `pay.vnpaytest.vn/dev-registration`
    (TmnCode và HashSecret gửi qua email), MoMo lấy ở cổng M4B. Điểm tài liệu chưa rõ, cần thử
    thật: thuật toán chữ ký của querydr và refund VNPAY (đang dùng HMAC-SHA512 như URL thanh toán),
    địa chỉ production của VNPAY, `orderId` trong truy vấn hoàn tiền MoMo, đơn vị `orderExpireTime`.
    IPN của VNPAY khai trong cổng merchant (không gửi theo từng giao dịch):
    `{FECO_PAYMENT_CALLBACK_BASE}v1/payments/vnpay/ipn`.
66. **Thử thật ZaloPay qua tunnel**: tạo đơn và chuyển sang trang sandbox chạy được, nhưng trình
    duyệt tích hợp chặn một tài nguyên của trang cổng nên tôi không trả được. Cần chủ dự án trả một
    giao dịch nhỏ trên trình duyệt của mình (ứng dụng ZaloPay sandbox hoặc thẻ thử) để kiểm callback
    thật và hoàn tiền. `FECO_PAYMENT_CALLBACK_BASE` cho phép callback qua tunnel hay staging.
67. **MISA meInvoice**: cần tài khoản (ClientID, ClientSecret ở developer.misa.vn; mã số thuế, ký
    hiệu hóa đơn đã đăng ký, chứng thư ký HSM). Chưa rõ trong tài liệu: cách đánh dấu điều chỉnh
    giảm (đang gửi `ReferenceType` 2 kèm số tiền dương của phần giảm), địa chỉ sandbox riêng (có vẻ
    theo tài khoản), định dạng `OrgInvDate`. Thuế suất đặt qua `FECO_MISA_VAT_RATE`.
68. **Bảng quản trị**: thành viên đầu tiên được thêm bằng `feco-catalog admin-set`; công cụ dòng lệnh
    vẫn làm được việc quản trị mà không qua passkey, nên chỉ người vận hành máy chủ được dùng (sẽ
    rút gọn khi bảng quản trị đủ chức năng). Kiểm passkey dựa trên `authentication_methods` của
    phiên Kratos (cần `webauthn`). Công cụ chạy thử app trong máy ảo chờ task 9.1.
69. **Thông báo** (đã xử lý phần tiếng Anh 13/11/2026: ngôn ngữ lấy từ trait `locale`, mọi thông
    báo người dùng có bản tiếng Anh; xem tasks.md 14.3): ngôn ngữ theo `traits.language`; mới có bản tiếng Anh cho báo giá, tin nhắn, cập
    nhật app, các thông báo khác vẫn tiếng Việt (task 15.1). Email cảnh báo bảo mật (đăng nhập lạ)
    vẫn gửi thẳng, chưa vào trung tâm thông báo. Liên kết hủy đăng ký không hết hạn.
70. **SPF, DKIM, DMARC**: chưa có tên miền. Khi có: chọn máy chủ chuyển tiếp SMTP ký DKIM (nhà cung
    cấp email giao dịch hoặc Postfix kèm OpenDKIM), đặt `FECO_SMTP_URL` dạng `smtps://`, thêm bản ghi
    theo `feco-catalog mail-dns <tên miền> <selector>` cho tới khi báo "đạt".

## Thay đổi improve-launcher-experience (giao diện launcher, app từ GitHub)

71. **Renderer của launcher đổi từ glow (OpenGL) sang wgpu.** Trên máy thử (Windows 10, NVIDIA RTX
    3060), eframe 0.37 với glow vẽ cửa sổ trắng hoàn toàn, kể cả bản F-Launcher 1.0.0 đã phát hành
    và một app eframe tối giản. wgpu (DirectX 12, Vulkan hoặc Metal) vẽ đúng. Cái giá là tệp chạy
    lớn hơn và nhiều thư viện phụ thuộc hơn (đều MIT hoặc Apache-2.0). Chủ dự án đồng ý phát hành
    bản vá: 1.0.1 (nhánh `release/1.0`) chỉ gồm bản sửa renderer và giờ địa phương.
72. **Biểu tượng app.** Launcher dùng ô chữ cái đầu có màu theo mã app. Icon do nhà phát triển tải
    lên cần thêm một target TUF đã ký cho mỗi icon, và quy định kích thước, định dạng, kiểm duyệt.
    Có làm ở thay đổi sau không?
73. **App mã nguồn mở do người khác đóng gói.** Spec yêu cầu kèm giấy phép gốc. Chưa có quy định về
    tên và nhãn hiệu (ví dụ một nhóm đóng gói lại app nổi tiếng dưới tên gốc), hay việc có cần dự
    án gốc đồng ý không. Có cần chính sách nội dung riêng cho trường hợp này không?
74. **App terminal trên macOS** mở qua `open -a Terminal` nên không nhận được mã khởi chạy (biến môi
    trường không đi qua). Hiện `f-dev` chỉ cảnh báo. Có muốn launcher ghi mã ra một file tạm chỉ
    app đọc được không? Cách này cần thêm thiết kế bảo mật.
75. **Tên họa sĩ trong phong cách giao diện** (đã xử lý 14/11/2026: đổi sang tên phổ thông theo yêu
    cầu chủ dự án, thay đổi platform-roles) (launcher-personalization). Phong cách chỉ dùng
    bảng màu do F-Eco tự chọn, ghi "cảm hứng từ bảng màu", không dùng hình ảnh tác phẩm. Với họa
    sĩ Việt Nam (Tô Ngọc Vân, Bùi Xuân Phái, Nguyễn Gia Trí), quyền nhân thân về tên và quyền của
    người thừa kế cần luật sư xác nhận trước khi phát hành thương mại. Mã cài đặt (`to_ngoc_van`,
    …) không đổi nếu cần đổi tên hiển thị.
76. **Thông báo lỗi kỹ thuật chưa dịch.** Giao diện launcher có đủ tiếng Anh và tiếng Pháp, nhưng
    lỗi từ thư viện bên dưới (TUF, mạng, cài đặt gói) vẫn tiếng Việt. Có cần dịch luôn ở bản sau
    không?
77. **Sandbox MoMo trả 403 Forbidden** (13/11/2026). `test-payment.momo.vn` trả trang lỗi 403 của
    nginx cho mọi yêu cầu từ máy phát triển, kể cả trang gốc và khi dùng khóa thử công khai trong
    tài liệu MoMo. Có thể cổng chỉ mở cho địa chỉ IP đã đăng ký với tài khoản MoMo Business (M4B).
    Cần chủ dự án đăng ký M4B, khai địa chỉ IP máy chủ thử, rồi chạy
    `feco-catalog payments-check` và `cargo test -p feco-server --test momo_sandbox`
    (docs/payments-setup.md mục 4).
78. **Rà soát bản dịch tiếng Nhật** (web-ui-and-japanese). Chuỗi tiếng Nhật của F-Launcher
    (`clients/feco-launcher/src/i18n_ja.rs`) và trang web (`services/feco-server/src/i18n_ja.rs`)
    do nhóm phát triển dịch từ bản tiếng Anh, chưa có người bản ngữ đọc lại. Thông báo của Kratos
    và email vẫn tiếng Anh với người dùng chọn tiếng Nhật. Cần người bản ngữ rà soát trước khi mở
    cho người dùng Nhật, và quyết định có dịch email không.
79. **Tiếng Pháp trên web.** F-Launcher có tiếng Pháp nhưng trang web chỉ có tiếng Việt, tiếng Anh
    và tiếng Nhật (theo yêu cầu thay đổi web-ui-and-japanese). Có cần thêm tiếng Pháp cho web để
    hai bên đồng nhất không?
80. **Đếm người dùng trên dashboard** gọi API danh sách danh tính của Kratos mỗi lần mở trang Tổng
    quan. Ổn với số người dùng nhỏ; khi lớn cần đếm định kỳ hoặc lưu số liệu. Chọn cách nào khi lên
    production?
