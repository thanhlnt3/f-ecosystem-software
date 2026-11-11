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
