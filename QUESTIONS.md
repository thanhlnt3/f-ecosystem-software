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
   Cập nhật 1.13.0: phần lập trình đã xong và được kiểm với nhà cung cấp OIDC giả. Chỉ còn đặt
   `SELFSERVICE_METHODS_OIDC_CONFIG_PROVIDERS` cho Kratos khi có client ID thật (README, mục Cấu hình).
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
72. **Biểu tượng app** (đã xử lý 15/11/2026: logo PNG tối đa 64 KB được ký trong metadata TUF và lưu trong danh mục, thay đổi launcher-logos-and-updates). Launcher dùng ô chữ cái đầu có màu theo mã app. Icon do nhà phát triển tải
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
79. **Tiếng Pháp trên web** (đã xử lý 15/11/2026: thêm tiếng Pháp cho web theo yêu cầu chủ dự án, thay đổi launcher-windows-and-web-profile; bản dịch chờ người bản ngữ rà soát như mục 78). F-Launcher có tiếng Pháp nhưng trang web chỉ có tiếng Việt, tiếng Anh
    và tiếng Nhật (theo yêu cầu thay đổi web-ui-and-japanese). Có cần thêm tiếng Pháp cho web để
    hai bên đồng nhất không?
80. **Đếm người dùng trên dashboard** (đã xử lý 15/11/2026: giữ số đếm 10 phút trong bộ nhớ của máy chủ, thay đổi launcher-windows-and-web-profile; khi chạy nhiều máy chủ, mỗi máy đếm riêng, chấp nhận được với con số tổng quan) gọi API danh sách danh tính của Kratos mỗi lần mở trang Tổng
    quan. Ổn với số người dùng nhỏ; khi lớn cần đếm định kỳ hoặc lưu số liệu. Chọn cách nào khi lên
    production?
81. **Giám sát từ bên ngoài cho trang trạng thái** (tasks.md 15.3). `/status` chạy trong
    feco-server nên không hiện được khi máy chủ hay cơ sở dữ liệu ngừng hẳn. Production cần một
    dịch vụ giám sát đặt ở nơi khác (tự dựng hay dịch vụ thuê) gọi `/healthz` và `/status.json`
    mỗi phút và báo cho người trực. Chọn dịch vụ nào, ai trực?
82. **Rà soát pháp lý về lưu trữ và chuyển dữ liệu ra nước ngoài** (tasks.md 15.4). Bản nháp
    `docs/legal/data-inventory.md` liệt kê dữ liệu, nơi lưu và các luồng có thể ra nước ngoài (email,
    AI, hạ tầng chưa chọn). Cần luật sư xác nhận văn bản áp dụng, lập hồ sơ đánh giá tác động và
    hồ sơ chuyển dữ liệu ra nước ngoài trước khi ra mắt. Task 15.4 chưa đánh dấu xong.
83. **Kiểm thử xâm nhập độc lập** (tasks.md 15.6) cần một đơn vị bên ngoài. Phạm vi đề xuất:
    trang web và API của feco-server, luồng OAuth/OIDC, F-Launcher (cập nhật TUF, mã khởi chạy,
    API cục bộ), cổng nhà phát triển (tải gói), webhook thanh toán. Chủ dự án chọn đơn vị và thời
    điểm nào?
84. **Kiểm thử tải trên máy chủ thật.** Biên bản ngày 15/11/2026 đo trên một máy dev (máy tạo tải
    và máy chủ chung một máy, Kratos và Hydra dùng SQLite). Cần chạy lại trên hạ tầng staging giống
    production trước khi ra mắt.

## Thay đổi fcoin-wallet-and-marketplace và launcher-logos-and-updates (bản 1.6.0)

85. **Pháp lý của F-Coin.** F-Coin không phải tiền mã hóa hay tài sản số; đó là số dư trả trước chỉ
    dùng trong F-Eco. Nhưng F-Coin rút được ra tiền thật (hoàn về giao dịch nạp, hoặc chuyển khoản).
    Vì vậy cần luật sư xác nhận trước khi mở cho người dùng thật:
    - có cần giấy phép dịch vụ trung gian thanh toán hoặc ví điện tử không (Nghị định 52/2024/NĐ-CP
      về thanh toán không dùng tiền mặt), hay phải giới hạn F-Coin là "điểm" không đổi ngược ra tiền;
    - xuất hóa đơn lúc nạp hay lúc dùng F-Coin; thuế của tiền bán app và tiền dự án trả cho lập
      trình viên;
    - nghĩa vụ phòng chống rửa tiền: giới hạn nạp, rút (hiện 10–50.000 F-Coin mỗi lần nạp, rút từ
      10 F-Coin, từ 10.000.000 ₫ cần hai người duyệt), định danh khách hàng (KYC).

    Trong lúc chờ, đặt `FECO_WALLET_WITHDRAWALS=false` để tắt việc tạo lệnh rút mới.
86. **Chi hộ qua cổng thanh toán.** Rút về ví MoMo hoặc ZaloPay của người dùng (không phải hoàn về
    giao dịch nạp) cần hợp đồng dịch vụ chi hộ với cổng. Hiện cách "chuyển khoản ngân hàng" do bộ
    phận Tài chính tự chuyển rồi ghi mã chuyển khoản. Có ký hợp đồng chi hộ với MoMo hoặc ZaloPay
    không?
87. **Hợp đồng dự án.** Hợp đồng hiện là thỏa thuận điện tử có lưu vết: nội dung, SHA-256, người ký
    (đánh dấu đồng ý và nhập họ tên), thời điểm ký. Cần luật sư xác nhận hai việc:
    - giá trị pháp lý theo Luật Giao dịch điện tử 2023, và doanh nghiệp có cần chữ ký số có chứng
      thư (CA) không;
    - rà soát mẫu hợp đồng: phạm vi, nghiệm thu, sở hữu mã nguồn, tranh chấp.
88. **Ví của tổ chức.** Ví F-Coin gắn với từng tài khoản người dùng. Tiền bán app của một công ty vào
    ví của chủ (owner) tài khoản nhà phát triển. Có cần ví riêng cho tổ chức, nhiều người cùng quản lý
    và phân quyền chi tiêu không?
89. **Logo F-Launcher dựa trên nhận diện FPT.** Theo yêu cầu, logo F-Launcher là chữ F nghiêng ghép
    từ ba thanh cam, xanh lá, xanh dương, lấy cảm hứng từ bộ ba màu của Tập đoàn FPT. Logo không
    chép hình hay chữ của logo FPT. Dùng yếu tố nhận diện của FPT trong sản phẩm cần bộ phận thương
    hiệu của FPT đồng ý trước khi phát hành thương mại. Ai sẽ xin ý kiến này?
90. **Phí nền tảng.** Phí bán app và phí dự án đã làm xong nhưng đang đặt 0%. Chủ dự án muốn mức
    phí nào khi ra mắt, và phí có tính trên tiền chia khi xử lý tranh chấp không (hiện có)?

## Thay đổi launcher-sidebar-and-web-apps (bản 1.7.0)

91. **Giấy phép ThesisAI.** Hai repo thesisai-fe và thesisai-be chưa có tệp LICENSE. Chủ dự án xác
    nhận tác giả đồng ý cho F-Eco tích hợp, sửa phần đăng nhập và dùng logo. Đề nghị tác giả:
    - thêm LICENSE vào hai repo, ví dụ MIT;
    - xác nhận bằng văn bản (email hoặc issue trên GitHub) để lưu hồ sơ.
92. **Chạy ThesisAI thật.** Bản vá ThesisAI mới được kiểm bằng `py_compile` và áp thử, chưa chạy
    thật. Muốn chạy trên máy dev cần tải về:
    - Ollama và mô hình `qwen2.5:7b`, `bge-m3` (khoảng 6 GB);
    - pgvector cho PostgreSQL;
    - thư viện Python (khoảng 1,5 GB) và npm (khoảng 400 MB).

    Chủ dự án cho phép tải phần nào, hay tác giả ThesisAI tự chạy và báo kết quả? Câu hỏi thứ hai
    là ThesisAI sẽ chạy ở đâu khi ra mắt, để đổi `web_url` sang HTTPS.
93. **App web qua Cổng nhà phát triển.** Hiện app web được đưa vào bằng `feco-tuf` và
    `feco-catalog` (nhà vận hành). Có cần cho nhà phát triển tự gửi app web qua Cổng nhà phát
    triển không? Nếu có, kiểm duyệt app web cần gì (kiểm tên miền, chính sách quyền riêng tư)?

## Thay đổi role-at-registration (bản 1.8.0)

94. **Xác minh trước khi duyệt vai trò.** Trang Phân quyền ghi email đã xác minh hay chưa, nhưng
    vẫn cho duyệt yêu cầu của email chưa xác minh. Có nên chặn duyệt cho tới khi email được xác
    minh không? Có cần kiểm mã số thuế với cơ quan thuế (tra cứu mã số thuế doanh nghiệp) trước khi
    duyệt Khách hàng doanh nghiệp và Công ty phát triển phần mềm không?

## Thay đổi production-readiness (bản 1.9.0)

95. **Cảnh báo refresh token bị dùng lại (task 2.5).** Launcher đổi refresh token trực tiếp với
    Hydra, nên F-Eco không biết khi nào Hydra phát hiện token bị dùng lại. Cần một trong hai:
    - Hydra phát sự kiện (bản Ory Network có event stream);
    - hoặc đưa việc đổi token qua feco-server.

    Chọn cách nào?
96. **API công khai của Kratos ở production.** Giới hạn đăng nhập sai, thử thách chống bot và
    trang "đã gửi email" giống nhau chỉ nằm trong feco-server. Gọi thẳng `/self-service/*/api` của
    Kratos thì bỏ qua được các lớp này. Khi dựng hạ tầng production (task 1.4), reverse proxy chỉ
    được mở các đường `/self-service/*/browser` và các trang chuyển hướng của Kratos. Bộ đếm đăng
    nhập sai cũng nên chuyển sang Valkey khi chạy nhiều máy chủ.
97. **Gói của app có giá và app riêng tư.** Mã khởi chạy đã chỉ cấp cho người được dùng app. Nhưng
    kho TUF vẫn phục vụ gói của mọi app cho bất kỳ ai, kể cả app có giá và bản build riêng của khách
    hàng. Có cần phục vụ các gói này qua đường có xác thực, hoặc mã hóa theo quyền dùng không?
98. **Lệnh hoàn chưa rõ kết quả.** Khi cổng không trả lời hoặc chữ ký phản hồi sai, lệnh hoàn giữ
    trạng thái "đang xử lý" và quản trị viên nhận cảnh báo đối chiếu. Hiện chưa có nút "đánh dấu
    thất bại, trả F-Coin" trên trang quản trị. Lệnh hoàn VNPAY trả mã 94 cũng chưa được hỏi lại kết
    quả. Có cần thêm thao tác này cho Tài chính không?
99. **Quỹ nền tảng.** Cấp F-Coin từ quỹ nền tảng là phát hành thêm F-Coin không có tiền thật đối
    ứng. Cần kế toán xác nhận cách hạch toán (chi phí khuyến mại) và có đặt hạn mức cấp mỗi tháng
    không.
100. **Đăng xuất từ app con.** Bấm đăng xuất trong một app dùng đăng nhập F-Eco chỉ kết thúc phiên
     của app đó; phiên F-Eco trên trình duyệt vẫn còn, nên lần đăng nhập OIDC sau không hỏi mật
     khẩu. Trên máy dùng chung, có nên hỏi "Đăng xuất cả F-Eco?" và kết thúc luôn phiên F-Eco không?

## Thay đổi mobile-launcher (bản 1.10.0)

101. **Phát hành trên Google Play.** APK hiện ký bằng khóa debug, chỉ để cài thử. Đưa lên Google
     Play cần:
     - tài khoản Google Play Console (phí một lần 25 USD) của F-Eco;
     - khóa tải lên lưu trong kho bí mật (GitHub Secrets);
     - chính sách quyền riêng tư và mục "An toàn dữ liệu" trên Play.

     Ai tạo tài khoản và giữ khóa?
102. **Phát hành trên App Store.** Bản iOS hiện chỉ chạy trên Simulator. Cài lên iPhone, TestFlight
     hoặc App Store cần tài khoản Apple Developer (99 USD/năm), chứng thư phân phối và provisioning
     profile cho `vn.feco.launcher`. Mua app bằng F-Coin trong app iOS có thể bị xem là mua hàng số
     phải qua In-App Purchase (Điều 3.1.1 của App Review Guidelines). Cần xem xét trước khi gửi duyệt:
     ẩn nút mua trên iOS, hoặc chỉ mở trang web.
103. **App di động thật trong danh mục.** Hiện F-Store chỉ có app desktop và app web. Có cần thêm nền
     tảng `android`, `ios` để nhà phát triển đăng app di động (mở trang Google Play hoặc App Store
     từ F-Launcher di động) không?

## Thay đổi mentor-integration (bản 1.11.0)

104. **Giấy phép của Mentor.** Repo Mentor chưa có tệp LICENSE. Chủ dự án xác nhận tác giả cho phép F-Eco
     tải, dựng, sửa phần đăng nhập và dùng logo, có ghi nguồn. Tác giả có thể thêm LICENSE (ví dụ MIT
     hoặc Apache-2.0) vào repo để F-Store ghi đúng giấy phép không?
105. **Khóa FPT AI của Mentor.** Hỏi đáp, soạn khóa học và nghiên cứu của Mentor gọi FPT AI Factory
     (`FPT_API_KEY`). F-Eco không giữ khóa này: người chạy gateway Mentor tự điền. Khi Mentor chạy cho
     nhiều người dùng F-Eco, ai trả chi phí AI? Có tính vào F-Coin không?
106. **Vai trò giáo viên.** Đăng nhập bằng F-Eco chỉ tạo tài khoản học sinh. Có nên dựa vào vai trò F-Eco
     (ví dụ một vai trò "giáo viên" được quản trị F-Eco duyệt) để tự cấp vai trò giáo viên trong Mentor
     không? Hiện quản trị Mentor nâng vai trò bằng `set-role`.
107. **Gateway Mentor dùng chung.** App desktop và web hiện gọi gateway ở `http://localhost:8100`, nên
     gateway phải chạy trên cùng máy. Có cần máy chủ Mentor chung (HTTPS) cho người dùng F-Eco không?
     Nếu có thì đặt ở đâu, và ai vận hành?
108. **Ký số app desktop Mentor.** `Mentor.exe` chưa ký số, nên Windows SmartScreen có thể cảnh báo.
     Gói vẫn được xác minh bằng chữ ký TUF của F-Eco. Có cần chứng thư ký mã cho app bên thứ ba
     không?

## Thay đổi chat-and-source-scan (bản 1.12.0)

109. **Lỗ hổng nghiêm trọng của thư viện.** OSV xếp CRITICAL thì bộ quét vẫn chỉ ghi mức `high`
     (chuyển duyệt tay, không tự từ chối), vì app có thể không dùng tới đoạn mã lỗi. Quét ThesisAI
     (frontend, commit `8f3735e`) thấy Next.js 16.4.0 có hai lỗ hổng chạy mã từ xa không cần đăng nhập
     (GHSA-2464-fhr4-w9xw, GHSA-673q-mjm6-r5xr). **Đã xử lý (21/11/2026)**: `thesisai-fe.patch` nâng
     Next.js lên 16.4.6; quét lại không còn lỗ hổng Next.js (`integrations/thesisai/SCAN.md`). Còn mở:
     có nên tự từ chối khi OSV ghi CRITICAL không? Và ai báo tác giả ThesisAI áp bản vá mới?
110. **Repo mã nguồn không tải được.** Repo riêng tư hoặc commit không tồn tại (GitHub trả 404) hiện bị
     từ chối với `source_scan`. Có nên chuyển duyệt tay thay vì từ chối không?
111. **Thư viện chỉ dùng khi phát triển.** Lỗ hổng trong thư viện dev (test, công cụ dựng) hiện cũng
     chuyển duyệt tay. Có nên bỏ qua nhóm này không?
112. **`[source]` và liên kết mã nguồn công khai.** Mã nguồn khai bằng `[source]` chỉ dùng cho kiểm
     duyệt. Có nên hiện nó thành liên kết "Mã nguồn" trên trang app không?
113. **Tắt nhóm "Tin nhắn".** Hiện tắt nhóm chỉ dừng email; thông báo trong trung tâm thông báo vẫn có,
     giống các nhóm tùy chọn khác. Có nên dừng cả thông báo trong app không?
114. **Chủ app trong F-Store.** Nút "Nhắn nhà phát triển" chỉ hiện khi app thuộc một tài khoản nhà phát
     triển có chủ đang hoạt động. App đưa vào bằng `scripts/dev-apps.sh` (như Mentor) không có tài khoản
     nhà phát triển nên không có nút. Có nên cho quản trị gán người nhận tin cho các app này không?
115. **Hỗ trợ xem tệp đính kèm.** Quản trị Hỗ trợ xem hội thoại bị báo cáo ở chế độ chỉ đọc, nhưng
     không tải được tệp đính kèm. Có cần cho tải (tệp đã quét sạch) để xử lý báo cáo không?
116. **Nhiều máy chủ.** "Không báo khi đang mở hội thoại" chỉ đếm kết nối trên từng máy chủ. Khi chạy
     nhiều máy chủ, người đang xem trên máy chủ khác vẫn có thể nhận thông báo. Có cần lưu người đang
     xem vào cơ sở dữ liệu không?
117. **Thu hồi rồi dùng lại refresh token.** App tự gọi `/oauth2/revoke` của Hydra rồi lại gửi chính
     refresh token đó tới F-Eco sẽ làm người dùng nhận cảnh báo "có dấu hiệu bị sao chép" nhầm. App
     chuẩn không làm vậy, và F-Launcher chỉ thu hồi khi đăng xuất. Có cần chuyển cả `/oauth2/revoke`
     qua F-Eco để tránh trường hợp này không?
118. **Chặn gọi thẳng điểm nhận token của Hydra.** Khi triển khai thật, có nên để proxy phía trước chỉ
     mở `/oauth2/token` của F-Eco, không mở `/oauth2/token` của Hydra ra Internet không? Hiện gọi thẳng
     Hydra vẫn chạy; F-Eco vẫn cảnh báo khi chủ token gửi token đã bị dùng lén lên F-Eco, nhưng không
     thấy được lần dùng lén đó.
119. **Đăng nhập Google/GitHub khi đang đăng nhập vào app con.** Khi người dùng bấm "Đăng nhập bằng
     Google" ở màn hình đăng nhập của một app con (luồng Hydra), F-Eco vẫn ghi nhật ký và kiểm tra
     thiết bị mới. Có cần hiện lại màn hình đồng ý của app ngay sau lần liên kết tài khoản đầu tiên không?
120. **Kratos trong CI dùng cơ sở dữ liệu bộ nhớ.** Hai lần CI gặp lỗi 500 "Could not update flow" của
     Kratos (`DSN=memory`) khi nhiều test chạy cùng lúc; chạy lại thì qua. Có nên chuyển Kratos của CI
     sang SQLite trên đĩa hoặc PostgreSQL không?
121. **Thông báo của hệ điều hành và thông báo đẩy.** F-Launcher máy tính báo bản mới bằng hộp thoại,
     huy hiệu và trung tâm thông báo, chưa có thông báo của Windows/macOS. Điện thoại chỉ biết khi
     mở app hoặc quay lại từ nền, chưa có thông báo đẩy. Cả hai cần thư viện mới (ví dụ `notify-rust`;
     Firebase Cloud Messaging/APNs với khóa dịch vụ). Có cần không, và dùng dịch vụ đẩy nào?
122. **Một tài khoản, nhiều máy.** Máy chủ ghi "đã cài" theo tài khoản, không theo máy. Gỡ app ở một
     máy thì app rời "Ứng dụng của tôi" dù máy kia vẫn cài; máy kia báo lại khi cập nhật lần tới. Có
     cần ghi theo từng thiết bị không?
123. **Kênh beta.** F-Launcher chưa có lựa chọn nhận bản beta, nên bản beta không có thông báo và không
     hiện "Có bản mới". Có cần lựa chọn này (và thông báo cho người chọn beta) không?
124. **Phiên bản theo nền tảng.** Thông báo cập nhật gửi theo phiên bản, không theo nền tảng: bản 1.2.0
     chỉ có gói Windows vẫn báo cho người cài trên macOS. Máy chủ chưa biết người dùng cài trên nền
     tảng nào. Có cần ghi nền tảng khi F-Launcher báo đã cài không?
125. **Danh sách quản trị rất dài.** Bộ lọc chạy trên các dòng đã đọc: đơn 500 mới nhất, dự án 200,
     báo cáo tin nhắn 500. Khi vượt mức này, cần phân trang và lọc ngay trong cơ sở dữ liệu. Khi nào
     cần (ước lượng số đơn và dự án mỗi tháng)?
126. **Tìm người dùng theo một phần email.** Trang "Hỗ trợ người dùng" chỉ tìm theo đúng email hoặc
     mã đơn và che email, để nhân viên hỗ trợ không duyệt được danh sách người dùng. Có cần tìm theo
     một phần email hay tên (kèm ghi nhật ký) không?
127. **Bảng quản trị gốc trên máy tính và điện thoại.** Theo lựa chọn ngày 21/11/2026, F-Launcher và
     app di động mở trang web quản trị. Nếu sau này cần màn hình gốc (xem danh sách không cần trình
     duyệt), phải thêm API quản trị riêng có passkey. Có kế hoạch không?
128. **Tên miền gửi email khi ra mắt.** Máy dev dùng được Gmail với mật khẩu ứng dụng (khoảng 500
     email mỗi ngày). Khi ra mắt cần tên miền gửi riêng có SPF, DKIM, DMARC, và một dịch vụ gửi
     (ví dụ Amazon SES, Mailgun, Postmark) (task 14.3). Chọn dịch vụ nào, tên miền nào?
129. **Liên kết xác minh trên điện thoại.** Liên kết trong email xác minh trỏ về địa chỉ công khai
     của Kratos (`127.0.0.1` trên máy dev), nên mở trên điện thoại không được. Có nên đổi xác minh
     sang mã 6 số (như khôi phục mật khẩu) để nhập được từ bất kỳ thiết bị nào không?
130. **Đề nghị tác giả app con nâng thư viện.** Mentor còn `soupsieve` 2.10 cho Python dưới 3.11.5
     (nên nâng `requires-python`); ThesisAI upstream vẫn dùng Next.js 16.4.0 (bản vá của F-Eco nâng
     lên 16.4.6) và `zod` 3 qua `shadcn`. Ai liên hệ tác giả, và có cần F-Eco gửi pull request?
131. **Gửi email thật khi chạy test.** E2E đọc email trong Mailpit. Khi Kratos dev gửi qua Gmail
     (`.dev/smtp-url`), chạy E2E sẽ gửi thư thật tới địa chỉ thử `@example.com` (đã xảy ra một lần
     ngày 23/11/2026). README nhắc khởi động lại Kratos với Mailpit trước khi test. Có cần chặn hẳn
     (E2E kiểm Kratos đang gửi tới Mailpit, không thì dừng) không?
132. **Phát hành F-Launcher qua kho phân phối.** `release.yml` dựng gói F-Launcher nhưng chưa ký vào kho
     TUF production (chưa có kho production, task 1.4 và 5.1). Khi có, bước ký gói `feco-launcher`
     nên chạy tự động trong `release.yml` hay do người giữ khóa targets làm tay?
133. **Giấy phép BinhAI Office.** Repo https://github.com/binhtran89/Demo_CraftAnything chưa có tệp
     giấy phép; F-Eco tích hợp theo xác nhận rằng tác giả cho phép (23/11/2026). Có văn bản cho phép
     của tác giả không, và nên đề nghị tác giả thêm giấy phép nào (ví dụ MIT)?
134. **Nơi chạy BinhAI Office khi ra mắt.** App là trang tĩnh; máy dev phục vụ ở
     `http://localhost:5180/`. Khi ra mắt, F-Eco có tự phục vụ bản dựng (CDN của F-Eco) không, hay
     tác giả tự đặt ở đâu đó? Cần đổi `web_url` sang HTTPS.
