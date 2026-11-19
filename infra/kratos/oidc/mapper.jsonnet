// Ánh xạ claim của Google và GitHub sang trait F-Eco (spec account-identity, "Nhà cung cấp đăng
// nhập ngoài"). Kratos chuẩn hóa claim của cả hai nhà cung cấp, nên một tệp dùng chung.
// Chỉ nhận email mà nhà cung cấp đã xác minh; email đó được tính là đã xác minh trên F-Eco.
local claims = { email_verified: false } + std.extVar('claims');
local verified = 'email' in claims && claims.email_verified;

{
  identity: {
    traits: {
      [if verified then 'email' else null]: claims.email,
      [if 'name' in claims && claims.name != '' then 'name' else null]: std.substr(claims.name, 0, 100),
    },
    verified_addresses: std.prune([
      if verified then { via: 'email', value: claims.email },
    ]),
  },
}
