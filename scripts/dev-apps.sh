#!/usr/bin/env bash
# Đưa thêm app vào môi trường dev của F-Launcher (thay đổi improve-launcher-experience, task 3.2).
# Cần đã chạy `scripts/dev-launcher.sh setup` (kho TUF, khóa) và Hydra đang chạy.
#
#   scripts/dev-apps.sh samples     # app mẫu dùng F-SDK: Ghi chú (f-notes), Bấm giờ (f-timer)
#   scripts/dev-apps.sh github <owner/repo> <tag-hoặc-commit> --id ID --name TÊN --bin FILE \
#       --license SPDX [--summary MÔ-TẢ] [--category NHÓM] [--developer TÊN] [--terminal] \
#       [--description GIỚI-THIỆU] [--features "A|B|C"] [--website URL] [--notes GHI-CHÚ]
#       [--icon LOGO.png]
#                                   # tải mã nguồn đúng tag/commit, dựng, đóng gói kèm giấy phép
#                                   # gốc, ký TUF, ghi danh mục (xem docs/github-apps.md)
#   scripts/dev-apps.sh opensource  # ba app mã nguồn mở của danh mục dev: kibi, bottom,
#                                   # eframe template
#   scripts/dev-apps.sh all         # samples + opensource
#   scripts/dev-apps.sh icons       # đặt logo (assets/logos/<id>-128.png) cho app đã có trong kho
#                                   # TUF và danh mục dev
#   scripts/dev-apps.sh thesisai    # app web ThesisAI: client OAuth phía máy chủ (có secret), gói app
#                                   # web, logo, danh mục; in biến môi trường cho backend ThesisAI
#                                   # (integrations/thesisai/README.md)
#   scripts/dev-apps.sh binhai      # app web BinhAI Office (tĩnh, không đăng nhập): gói app web,
#                                   # danh mục (integrations/binhai-office/README.md)
#   scripts/dev-apps.sh mentor      # Mentor: client OAuth phía máy chủ, target web; có
#                                   # MENTOR_DESKTOP_ZIP thì thêm target desktop; logo, danh mục; in biến
#                                   # môi trường cho gateway Mentor (integrations/mentor/README.md)
#
# Mã nguồn tải về và bản dựng nằm ngoài repo, trong $FECO_EXT_DIR (mặc định ~/.feco-dev-ext), để
# Cargo không coi chúng là một phần workspace của F-Eco. Chỉ dùng cho dev: bản phát hành thật đi
# qua f-dev, kiểm duyệt và máy chủ ký (docs/integration-guide.md).
set -euo pipefail
# shellcheck source=dev-launcher.sh
source "$(dirname "$0")/dev-launcher.sh"

EXT="${FECO_EXT_DIR:-$HOME/.feco-dev-ext}"

need_repo() {
  [[ -f "$REPO/metadata/1.root.json" ]] || {
    echo "chưa có kho TUF dev: chạy scripts/dev-launcher.sh setup trước" >&2
    exit 1
  }
}

# Đóng gói thư mục $1 thành zip $2 (đường dẫn trong zip dùng '/', file chạy có quyền 755).
zip_dir() {
  "$PY" -I - "$1" "$2" <<'EOF'
import os, sys, zipfile
src, out = sys.argv[1:3]
with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
    for root, _, files in os.walk(src):
        for f in sorted(files):
            path = os.path.join(root, f)
            info = zipfile.ZipInfo(os.path.relpath(path, src).replace(os.sep, "/"))
            info.external_attr = 0o755 << 16
            info.compress_type = zipfile.ZIP_DEFLATED
            with open(path, "rb") as fh:
                z.writestr(info, fh.read())
EOF
}

# JSON an toàn cho một chuỗi.
json_str() { "$PY" -I -c 'import json,sys; print(json.dumps(sys.argv[1], ensure_ascii=False))' "$1"; }

# Nội dung trang chi tiết của app sắp phát hành (đặt trước khi gọi publish_app). WEB_URL khác rỗng
# thì app là app web: target nền tảng `web`, launcher mở địa chỉ này bằng trình duyệt.
DESCRIPTION="" FEATURES="" WEBSITE="" NOTES="" ICON="" WEB_URL=""

# publish_app ID TÊN VERSION THƯ-MỤC-GÓI FILE-CHẠY CLIENT-ID-TUF SUMMARY CATEGORY DEVELOPER LICENSE \
#             SOURCE TERMINAL CATALOG-CLIENT-ID
publish_app() {
  local id="$1" name="$2" version="$3" pkg="$4" entry="$5" tuf_client="$6" summary="$7"
  local category="$8" developer="$9" license="${10}" source_url="${11}" terminal="${12}"
  local catalog_client="${13}"
  local plat zipname zipfile
  plat="$(platform)"
  [[ -n "$WEB_URL" ]] && plat=web
  zipname="$id-$version-$plat.zip"
  zipfile="$DEV/uploads/$zipname"
  # Mỗi phiên bản chỉ ký một lần: chạy lại script thì không ký lại phiên bản đã có trong kho
  # TUF, nhưng vẫn ghi danh mục (để chạy lại được khi lần trước PostgreSQL chưa chạy).
  if grep -qs "\"$zipname\"" "$REPO"/metadata/*targets.json && [[ -f "$zipfile" ]]; then
    echo "$id $version đã có trong kho TUF, chỉ ghi danh mục" >&2
    catalog_entry
    return 0
  fi
  mkdir -p "$DEV/uploads"
  rm -f "$zipfile"
  zip_dir "$pkg" "$zipfile"
  local args=(publish --repo "$REPO" --file "$zipfile" --name "$zipname" --app "$id" --version "$version"
    --platform "$plat" --title "$name" --entry "$entry" --summary "$summary" --category "$category"
    --developer "$developer" --license "$license" --sign "$(RELEASE_KEYS)")
  [[ -n "$DESCRIPTION" ]] && args+=(--description "$DESCRIPTION")
  [[ -n "$FEATURES" ]] && args+=(--features "$FEATURES")
  [[ -n "$WEBSITE" ]] && args+=(--website "$WEBSITE")
  [[ -n "$NOTES" ]] && args+=(--release-notes "$NOTES")
  [[ -n "$ICON" ]] && args+=(--icon "$ICON")
  [[ -n "$WEB_URL" ]] && args+=(--web-url "$WEB_URL")
  [[ -n "$tuf_client" ]] && args+=(--client-id "$tuf_client")
  [[ -n "$source_url" ]] && args+=(--source "$source_url")
  [[ "$terminal" == "true" ]] && args+=(--terminal true)
  "$TUF" "${args[@]}"
  catalog_entry
}

# Ghi app và phiên bản vào danh mục F-Store (dùng biến cục bộ của publish_app).
catalog_entry() {
  local json="$DEV/catalog-$id.json" extra=""
  # Trường tùy chọn dựng trước: trong heredoc, dấu nháy bên trong ${VAR:+...} bị bash bỏ đi.
  [[ -n "$WEB_URL" ]] && extra+=",\"web_url\":$(json_str "$WEB_URL")"
  [[ -n "$ICON" ]] && extra+=",\"icon_file\":$(json_str "$(cygpath -m "$ICON" 2>/dev/null || echo "$ICON")")"
  cat > "$json" <<EOF
{"id":"$id","client_id":"$catalog_client","name":$(json_str "$name"),
 "developer":$(json_str "$developer"),"developer_verified":false,"first_party":false,
 "summary":$(json_str "$summary"),
 "description":$(json_str "${DESCRIPTION:-$summary}${source_url:+ Mã nguồn: $source_url}"),
 "category":"$category","price_vnd":0,"license":$(json_str "$license"),"screenshots":[]$extra}
EOF
  catalog add-app "$json"
  catalog release "$id" "$version" "$plat" "$(wc -c < "$zipfile" | tr -d ' ')" "$zipname" \
    "${NOTES:-Bản dựng dev lúc $(date '+%H:%M %d/%m/%Y')}"
}

# App mẫu dùng F-SDK: có client OAuth riêng (scope profile), launcher cấp mã khởi chạy.
samples() {
  need_repo
  build
  cargo build -q --release --manifest-path "$ROOT/Cargo.toml" -p f-notes -p f-timer >&2
  local version
  version="$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)-dev.$(date +%Y%m%d%H%M%S)"
  local spec
  for spec in "f-notes|Ghi chú|Ghi chú nhanh, tự lưu, tìm trong mọi ghi chú.|productivity" \
              "f-timer|Bấm giờ|Đồng hồ bấm giờ có vòng và hẹn giờ tập trung.|productivity"; do
    IFS='|' read -r id name summary category <<<"$spec"
    ensure_client "$id-client-id" "{\"client_name\":$(json_str "$name"),\"grant_types\":[\"authorization_code\",\"refresh_token\"],
      \"response_types\":[\"code\"],\"scope\":\"openid offline_access profile\",
      \"redirect_uris\":[\"http://127.0.0.1:47280/callback\"],\"token_endpoint_auth_method\":\"none\",
      \"subject_type\":\"pairwise\",\"policy_uri\":\"https://f-eco.example/privacy\",
      \"metadata\":{\"developer\":\"F-Eco (app mẫu)\",\"scopes\":{
        \"profile\":{\"required\":true,\"purpose_vi\":\"Chào bạn bằng tên trong app\",\"purpose_en\":\"Greet you by name\"}}}}"
    local client pkg exe
    client="$(cat "$DEV/$id-client-id")"
    pkg="$EXT/pkg/$id"
    rm -rf "$pkg"
    mkdir -p "$pkg/bin"
    exe="$(exe "$TARGET_DIR/release/$id")"
    cp "$exe" "$pkg/bin/"
    cp "$ROOT/LICENSE-MIT" "$ROOT/LICENSE-APACHE" "$pkg/"
    WEBSITE="https://github.com/thanhlnt3/f-ecosystem-software/tree/main/examples/apps/$id"
    ICON="$ROOT/assets/logos/$id-128.png"
    NOTES="Bản dựng dev lúc $(date '+%H:%M %d/%m/%Y') từ mã nguồn hiện tại của examples/apps/$id."
    case "$id" in
      f-notes)
        DESCRIPTION="Ghi chú là app mẫu của F-Eco, viết bằng Rust và egui. Khi được F-Launcher mở, app tự đăng nhập bằng tài khoản F-Eco qua F-SDK (không cần nhập mật khẩu) và chào bạn bằng tên. Ghi chú lưu trong thư mục dữ liệu riêng mà launcher cấp cho app."
        FEATURES="Tạo, sửa, xóa ghi chú|Tự lưu sau khi ngừng gõ|Tìm trong tiêu đề và nội dung|Đăng nhập một lần qua F-Launcher (F-SDK)|Chạy chế độ khách khi mở trực tiếp" ;;
      f-timer)
        DESCRIPTION="Bấm giờ là app mẫu của F-Eco gồm đồng hồ bấm giờ có ghi vòng và hẹn giờ tập trung (làm việc rồi nghỉ). Khi được F-Launcher mở, app tự đăng nhập bằng tài khoản F-Eco qua F-SDK."
        FEATURES="Đồng hồ bấm giờ có tạm dừng và ghi vòng|Hẹn giờ tập trung, tự chuyển sang giờ nghỉ|Chỉnh thời lượng tập trung và nghỉ|Đếm số phiên tập trung đã hoàn thành|Đăng nhập một lần qua F-Launcher (F-SDK)" ;;
    esac
    publish_app "$id" "$name" "$version" "$pkg" "bin/$(basename "$exe")" "$client" "$summary" \
      "$category" "F-Eco" "MIT OR Apache-2.0" "" "false" "$client"
  done
}

# Tải, dựng và phát hành một app mã nguồn mở từ GitHub.
github() {
  need_repo
  local repo="${1:?cần owner/repo}" ref="${2:?cần tag hoặc commit}"
  shift 2
  local id="" name="" bin="" license="" summary="" category="other" developer="" terminal="false"
  DESCRIPTION="" FEATURES="" WEBSITE="" NOTES="" ICON=""
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --id) id="$2"; shift 2 ;;
      --name) name="$2"; shift 2 ;;
      --bin) bin="$2"; shift 2 ;;
      --license) license="$2"; shift 2 ;;
      --summary) summary="$2"; shift 2 ;;
      --category) category="$2"; shift 2 ;;
      --developer) developer="$2"; shift 2 ;;
      --terminal) terminal="true"; shift ;;
      --description) DESCRIPTION="$2"; shift 2 ;;
      --features) FEATURES="$2"; shift 2 ;;
      --website) WEBSITE="$2"; shift 2 ;;
      --notes) NOTES="$2"; shift 2 ;;
      --icon) ICON="$2"; shift 2 ;;
      *) echo "tham số lạ: $1" >&2; exit 2 ;;
    esac
  done
  [[ -n "$id" && -n "$name" && -n "$bin" && -n "$license" ]] || {
    echo "cần --id, --name, --bin, --license" >&2
    exit 2
  }
  developer="${developer:-${repo%%/*}}"
  build
  local src="$EXT/src/$id" out="$EXT/build/$id" pkg="$EXT/pkg/$id"
  # 1. Tải mã nguồn đúng tag hoặc commit (không dùng nhánh: bản dựng phải lặp lại được).
  if [[ ! -f "$src/.feco-ref" || "$(cat "$src/.feco-ref")" != "$repo@$ref" ]]; then
    rm -rf "$src"
    mkdir -p "$src"
    local url="https://codeload.github.com/$repo/tar.gz/$ref"
    echo "tải $url" >&2
    curl -sSfL "$url" | tar -xz -C "$src" --strip-components=1
    echo "$repo@$ref" > "$src/.feco-ref"
  fi
  # 2. Dựng bằng toolchain stable (bỏ qua rust-toolchain của dự án), đúng Cargo.lock của dự án.
  (cd "$src" && RUSTUP_TOOLCHAIN=stable CARGO_TARGET_DIR="$out" cargo build -q --release --locked) >&2
  # 3. Đóng gói: file chạy, giấy phép gốc, nguồn gốc.
  rm -rf "$pkg"
  mkdir -p "$pkg/bin" "$pkg/licenses"
  local exe
  exe="$(exe "$out/release/$bin")"
  cp "$exe" "$pkg/bin/"
  local found=0 f
  for f in "$src"/LICENSE* "$src"/LICENCE* "$src"/COPYING* "$src"/NOTICE* "$src"/COPYRIGHT*; do
    [[ -f "$f" ]] && cp "$f" "$pkg/licenses/" && found=1
  done
  [[ "$found" == 1 ]] || { echo "$repo không có file giấy phép ở thư mục gốc: dừng" >&2; exit 1; }
  printf 'Dựng từ https://github.com/%s tại %s\nGiấy phép: %s\n' "$repo" "$ref" "$license" \
    > "$pkg/SOURCE.txt"
  # 4. Client OAuth chỉ để có mục trong danh mục; target TUF không có client_id nên launcher mở
  #    app mà không xin mã khởi chạy (app không dùng F-SDK).
  ensure_client "$id-client-id" "{\"client_name\":$(json_str "$name"),\"grant_types\":[\"authorization_code\"],
    \"response_types\":[\"code\"],\"scope\":\"openid\",\"redirect_uris\":[\"http://127.0.0.1:9/unused\"],
    \"token_endpoint_auth_method\":\"none\",\"metadata\":{\"developer\":$(json_str "$developer")}}"
  local version="${ref#v}"
  [[ "$version" =~ ^[0-9]+\.[0-9]+ ]] || version="0.0.0-git.${ref:0:12}"
  publish_app "$id" "$name" "$version" "$pkg" "bin/$(basename "$exe")" "" "$summary" "$category" \
    "$developer" "$license" "https://github.com/$repo" "$terminal" "$(cat "$DEV/$id-client-id")"
}

opensource() {
  github ilai-deutel/kibi v0.3.3 --id kibi --icon "$ROOT/assets/logos/kibi-128.png" --name "kibi" --bin kibi --license "MIT OR Apache-2.0" \
    --summary "Trình soạn thảo văn bản siêu nhỏ chạy trong terminal (dưới 1.024 dòng mã)." \
    --category tools --developer "Ilaï Deutel" --terminal \
    --description "kibi là trình soạn thảo văn bản trong terminal, viết bằng Rust với chưa tới 1.024 dòng mã, lấy cảm hứng từ kilo. Bản trong F-Eco dựng từ mã nguồn gốc tại tag v0.3.3, không sửa đổi." \
    --features "Tô màu cú pháp cho hơn 30 ngôn ngữ|Tìm kiếm tăng dần|Sao chép, cắt, dán, nhân đôi dòng|Chạy lệnh ngoài và dán kết quả vào văn bản|Cấu hình qua tệp config.ini|Chạy trên Windows, macOS, Linux" \
    --notes "Bản 0.3.3 của dự án gốc: thêm Ctrl+/ để chú thích hoặc bỏ chú thích dòng; bật bảo vệ ngăn xếp cho bản phát hành. Chi tiết trong CHANGELOG.md của kho ilai-deutel/kibi."
  github ClementTsang/bottom 0.15.0 --id bottom --icon "$ROOT/assets/logos/bottom-128.png" --name "bottom" --bin btm --license "MIT" \
    --summary "Theo dõi CPU, bộ nhớ, mạng, ổ đĩa và tiến trình ngay trong terminal." \
    --category system --developer "Clement Tsang" --terminal --website "https://bottom.pages.dev/stable" \
    --description "bottom (lệnh btm) là công cụ theo dõi hệ thống đa nền tảng trong terminal, lấy cảm hứng từ gtop, gotop và htop. Bản trong F-Eco dựng từ mã nguồn gốc tại tag 0.15.0, không sửa đổi." \
    --features "Biểu đồ CPU, bộ nhớ, mạng theo thời gian|Danh sách tiến trình: lọc, sắp xếp, dừng tiến trình|Dung lượng và tốc độ đọc ghi ổ đĩa|Nhiệt độ cảm biến và pin|Giao diện tùy chỉnh bằng tệp cấu hình|Điều khiển bằng bàn phím và chuột" \
    --notes "Bản 0.15.0 của dự án gốc: hỗ trợ GPU Intel trên Linux, thanh dạng khối, đơn vị dung lượng nhị phân cho ổ đĩa, cột swap của tiến trình trên Linux; bật thanh cuộn mặc định; sửa lỗi khởi động trên Windows 7 và 8. Chi tiết trong CHANGELOG.md của kho ClementTsang/bottom."
  github emilk/eframe_template f108546a2b9f2fc6a82543ceae1625a3893305cb --id eframe-template --icon "$ROOT/assets/logos/eframe-template-128.png" \
    --name "eframe template" --bin eframe_template --license "MIT OR Apache-2.0" \
    --summary "App cửa sổ mẫu viết bằng egui/eframe, điểm khởi đầu cho app đồ họa bằng Rust." \
    --category development --developer "Emil Ernerfeldt" --website "https://www.egui.rs" \
    --description "eframe template là app mẫu của tác giả egui, dùng làm điểm khởi đầu cho app đồ họa viết bằng Rust chạy trên máy tính và trên web. Bản trong F-Eco dựng từ commit f108546, không sửa đổi." \
    --features "Cửa sổ đồ họa egui với chủ đề sáng và tối|Dựng cho máy tính và cho web (WebAssembly)|Mã ngắn, dễ sao chép làm app mới" \
    --notes "Bản tại commit f108546 của dự án gốc, dùng egui và eframe 0.36."
}

# Đặt logo cho các app đã phát hành trong môi trường dev (kho TUF và danh mục F-Store).
icons() {
  need_repo
  build
  local id json
  for id in f-notes f-timer kibi bottom eframe-template; do
    [[ -f "$DEV/catalog-$id.json" ]] || continue
    "$TUF" set-icon --repo "$REPO" --app "$id" --icon "$ROOT/assets/logos/$id-128.png" \
      --sign "$(RELEASE_KEYS)"
    json="$DEV/catalog-$id.json"
    "$PY" -I - "$json" "$(cygpath -m "$ROOT/assets/logos/$id-128.png" 2>/dev/null || echo "$ROOT/assets/logos/$id-128.png")" <<'PYEOF'
import json, sys
path, icon = sys.argv[1:3]
data = json.load(open(path, encoding="utf-8"))
data["icon_file"] = icon
json.dump(data, open(path, "w", encoding="utf-8"), ensure_ascii=False)
PYEOF
    catalog add-app "$json"
  done
}

# App web ThesisAI (thay đổi launcher-sidebar-and-web-apps). Mã nguồn ThesisAI không nằm trong
# gói: gói chỉ có thông tin app; ThesisAI chạy riêng ở http://localhost:3000 (frontend) và
# http://localhost:8000 (backend) sau khi áp bản vá trong integrations/thesisai/.
thesisai() {
  need_repo
  build
  local id=thesisai name="ThesisAI" version="${THESISAI_VERSION:-0.1.0}"
  local front="${THESISAI_URL:-http://localhost:3000}" back="${THESISAI_API:-http://localhost:8000}"
  local secret_file="$DEV/thesisai-client-secret" id_file="$DEV/thesisai-client-id" client
  # Client phía máy chủ: secret do script tạo, chỉ lưu trong .dev/ (không commit).
  [[ -s "$secret_file" ]] || "$PY" -I -c 'import secrets; print(secrets.token_urlsafe(32))' > "$secret_file"
  client="$(cat "$id_file" 2>/dev/null || true)"
  if [[ -z "$client" ]] || ! curl -sf -o /dev/null "$HYDRA_ADMIN/admin/clients/$client"; then
    create_client "{\"client_name\":\"ThesisAI\",\"client_secret\":$(json_str "$(cat "$secret_file" | tr -d '\r\n')"),
      \"grant_types\":[\"authorization_code\",\"refresh_token\"],\"response_types\":[\"code\"],
      \"scope\":\"openid offline_access profile email\",
      \"redirect_uris\":[\"$back/api/auth/feco/callback\"],\"token_endpoint_auth_method\":\"client_secret_basic\",
      \"subject_type\":\"pairwise\",\"policy_uri\":\"$front/\",
      \"metadata\":{\"developer\":\"justAchillGuy8386\",\"scopes\":{
        \"profile\":{\"required\":true,\"purpose_vi\":\"Hiện tên của bạn trong ThesisAI\",\"purpose_en\":\"Show your name in ThesisAI\"},
        \"email\":{\"required\":true,\"purpose_vi\":\"Tạo hoặc liên kết tài khoản ThesisAI của bạn\",\"purpose_en\":\"Create or link your ThesisAI account\"}}}}" \
      > "$id_file"
    client="$(cat "$id_file")"
  fi
  local pkg="$EXT/pkg/$id"
  rm -rf "$pkg"
  mkdir -p "$pkg"
  "$PY" -I -c 'import json,sys; json.dump({"name": sys.argv[1], "url": sys.argv[2]}, open(sys.argv[3], "w", encoding="utf-8"), ensure_ascii=False)' \
    "$name" "$front/auth/feco" "$pkg/webapp.json"
  printf 'ThesisAI: https://github.com/justAchillGuy8386/thesisai-fe và thesisai-be\nGói này chỉ chứa thông tin app web; ứng dụng chạy trong trình duyệt.\n' > "$pkg/NOTICE.txt"
  WEB_URL="$front/auth/feco"
  ICON="$ROOT/assets/logos/thesisai-128.png"
  WEBSITE=""
  DESCRIPTION="ThesisAI giúp sinh viên luyện bảo vệ đồ án tốt nghiệp với hội đồng AI chạy cục bộ: tải tài liệu đồ án lên, hệ thống sinh câu hỏi phản biện, sinh viên trả lời bằng giọng nói và nhận đánh giá. Ứng dụng web chạy trong trình duyệt và đăng nhập bằng tài khoản F-Eco."
  FEATURES="Tải tài liệu đồ án (PDF, DOCX, PPTX)|Sinh câu hỏi phản biện từ tài liệu (RAG)|Luyện trả lời bằng giọng nói, chuyển thành văn bản|Đánh giá câu trả lời và hồ sơ kiến thức|Mô hình AI chạy cục bộ qua Ollama|Đăng nhập bằng tài khoản F-Eco"
  NOTES="Bản tích hợp đầu tiên với F-Eco: đăng nhập bằng tài khoản F-Eco (bản vá trong integrations/thesisai)."
  publish_app "$id" "$name" "$version" "$pkg" "webapp.json" "" \
    "Luyện bảo vệ đồ án tốt nghiệp với hội đồng AI." education "justAchillGuy8386" \
    "Chưa công bố giấy phép (tác giả cho phép F-Eco tích hợp)" "https://github.com/justAchillGuy8386/thesisai-fe" \
    "false" "$client"
  WEB_URL=""
  cat >&2 <<ENVEOF

Đã đưa ThesisAI vào kho TUF và danh mục dev. Đặt các biến sau trong .env của backend ThesisAI
(client secret nằm trong $secret_file, không commit):
  FECO_ISSUER_URL=${HYDRA_PUBLIC:-http://127.0.0.1:4444}/
  FECO_SERVER_URL=${FECO_SERVER_URL:-http://127.0.0.1:8080/}
  FECO_CLIENT_ID=$client
  FECO_CLIENT_SECRET=<nội dung tệp $secret_file>
  FECO_REDIRECT_URI=$back/api/auth/feco/callback
  FRONTEND_URL=$front
ENVEOF
}

# App web BinhAI Office (thay đổi binhai-office-integration): app tĩnh, không cần đăng nhập. Gói
# chỉ có thông tin app; bản dựng từ mã nguồn (integrations/binhai-office/build.sh) được phục vụ ở
# http://localhost:5180 (đặt BINHAI_URL để đổi).
binhai() {
  need_repo
  build
  local id=binhai-office name="BinhAI Office" version="${BINHAI_VERSION:-0.1.0}"
  local front="${BINHAI_URL:-http://localhost:5180/}"
  local pkg="$EXT/pkg/$id"
  rm -rf "$pkg"
  mkdir -p "$pkg"
  "$PY" -I -c 'import json,sys; json.dump({"name": sys.argv[1], "url": sys.argv[2]}, open(sys.argv[3], "w", encoding="utf-8"), ensure_ascii=False)' \
    "$name" "$front" "$pkg/webapp.json"
  printf 'BinhAI Office: https://github.com/binhtran89/Demo_CraftAnything\nGói này chỉ chứa thông tin app web; ứng dụng chạy trong trình duyệt.\n' > "$pkg/NOTICE.txt"
  WEB_URL="$front"
  ICON=""
  WEBSITE=""
  DESCRIPTION="BinhAI Office là bản thử giao diện của một phòng AI làm việc trên máy tính: người dùng ra lệnh bằng tiếng Việt, 18 agent chia việc theo 8 quy trình, tự kiểm tra rồi gửi kết quả chờ duyệt, và văn phòng 3D phản ánh trạng thái công việc. Bản này chạy hoàn toàn trong trình duyệt với dữ liệu mô phỏng, không gọi dịch vụ AI và không cần tài khoản."
  FEATURES="Ra lệnh bằng tiếng Việt, tự phân loại quy trình|18 agent, 8 quy trình, bước kiểm tra chất lượng và phê duyệt|Văn phòng 3D theo dõi công việc|Xuất kết quả ra Markdown, Word, Excel|Dữ liệu lưu trên trình duyệt, không gửi ra ngoài"
  NOTES="Bản tích hợp đầu tiên: dựng từ mã nguồn tại commit 86a1fa3, đã quét bảo mật (integrations/binhai-office/SCAN.md)."
  publish_app "$id" "$name" "$version" "$pkg" "webapp.json" "" \
    "Bản thử phòng AI với 18 agent và văn phòng 3D, chạy trong trình duyệt." productivity "binhtran89" \
    "Chưa công bố giấy phép (tác giả cho phép F-Eco tích hợp)" "https://github.com/binhtran89/Demo_CraftAnything" \
    "false" "web-binhai-office"
  WEB_URL=""
  cat >&2 <<ENVEOF

Đã đưa BinhAI Office vào kho TUF và danh mục dev. Dựng và phục vụ app (integrations/binhai-office):
  integrations/binhai-office/build.sh "$EXT/binhai-office/site" --verify-dist
  python -m http.server 5180 --bind 127.0.0.1 --directory "$EXT/binhai-office/site"
ENVEOF
}

# Mentor (thay đổi mentor-integration): một app, hai target. Target `web` mở thẳng đăng nhập F-Eco
# của gateway Mentor; target desktop (Windows) lấy từ gói `mentor-<phiên bản>-windows-x64.zip` do CI
# của F-Eco dựng (đặt MENTOR_DESKTOP_ZIP). Mã Mentor không nằm trong gói web.
mentor() {
  need_repo
  build
  local id=mentor name="Mentor" version="${MENTOR_VERSION:-0.2.2}"
  local front="${MENTOR_WEB:-http://localhost:5173}" back="${MENTOR_API:-http://localhost:8100}"
  local secret_file="$DEV/mentor-client-secret" id_file="$DEV/mentor-client-id" client
  # Client phía máy chủ: secret do script tạo, chỉ lưu trong .dev/ (không commit).
  [[ -s "$secret_file" ]] || "$PY" -I -c 'import secrets; print(secrets.token_urlsafe(32))' > "$secret_file"
  client="$(cat "$id_file" 2>/dev/null || true)"
  if [[ -z "$client" ]] || ! curl -sf -o /dev/null "$HYDRA_ADMIN/admin/clients/$client"; then
    create_client "{\"client_name\":\"Mentor\",\"client_secret\":$(json_str "$(cat "$secret_file" | tr -d '\r\n')"),
      \"grant_types\":[\"authorization_code\"],\"response_types\":[\"code\"],
      \"scope\":\"openid profile\",
      \"redirect_uris\":[\"$back/api/auth/feco/callback\"],\"token_endpoint_auth_method\":\"client_secret_basic\",
      \"subject_type\":\"pairwise\",\"policy_uri\":\"$front/\",
      \"metadata\":{\"developer\":\"katD2707\",\"scopes\":{
        \"profile\":{\"required\":true,\"purpose_vi\":\"Hiện tên của bạn trong Mentor\",\"purpose_en\":\"Show your name in Mentor\"}}}}" \
      > "$id_file"
    client="$(cat "$id_file")"
  fi
  local pkg="$EXT/pkg/$id"
  local license="Chưa công bố giấy phép (tác giả cho phép F-Eco tích hợp)"
  local source="https://github.com/katD2707/agent-mentor-maximum-experience"
  local summary="Học tập cho học sinh lớp 1 tới đại học và giáo viên: khóa học, luyện tập, hỏi đáp, kế hoạch học."
  ICON="$ROOT/assets/logos/mentor-128.png"
  WEBSITE=""
  DESCRIPTION="Mentor là ứng dụng học tập cho học sinh từ lớp 1 tới đại học và cho giáo viên. Học sinh có khóa học, luyện tập, hỏi đáp với trợ lý Mực và theo dõi tiến độ; giáo viên đưa tài liệu lên và duyệt khóa học do AI soạn. Đăng nhập bằng tài khoản F-Eco. Dùng trên trình duyệt, hoặc cài app desktop trên Windows."
  FEATURES="Khóa học theo độ tuổi, bài học từng bước|Luyện tập có ôn tập ngắt quãng|Hỏi đáp với trợ lý Mực|Kế hoạch học và nhắc giờ học|Giáo viên soạn khóa học cùng AI|Đăng nhập bằng tài khoản F-Eco"
  NOTES="Bản tích hợp đầu tiên với F-Eco: đăng nhập bằng tài khoản F-Eco (bản vá trong integrations/mentor)."
  # Target desktop trước: catalog_entry của target web ghi web_url sau cùng.
  if [[ -n "${MENTOR_DESKTOP_ZIP:-}" ]]; then
    [[ -f "$MENTOR_DESKTOP_ZIP" ]] || { echo "không thấy $MENTOR_DESKTOP_ZIP" >&2; exit 1; }
    rm -rf "$pkg"
    mkdir -p "$pkg"
    "$PY" -I -c 'import sys, zipfile; zipfile.ZipFile(sys.argv[1]).extractall(sys.argv[2])' "$MENTOR_DESKTOP_ZIP" "$pkg"
    [[ -f "$pkg/Mentor.exe" ]] || { echo "gói desktop thiếu Mentor.exe" >&2; exit 1; }
    WEB_URL=""
    publish_app "$id" "$name" "$version" "$pkg" "Mentor.exe" "" "$summary" education "katD2707" \
      "$license" "$source" "false" "$client"
  fi
  rm -rf "$pkg"
  mkdir -p "$pkg"
  WEB_URL="$back/api/auth/feco/login?return=$front"
  "$PY" -I -c 'import json,sys; json.dump({"name": sys.argv[1], "url": sys.argv[2]}, open(sys.argv[3], "w", encoding="utf-8"), ensure_ascii=False)' \
    "$name" "$WEB_URL" "$pkg/webapp.json"
  printf 'Mentor: %s\nGói này chỉ chứa thông tin app web; ứng dụng chạy trong trình duyệt.\n' "$source" > "$pkg/NOTICE.txt"
  publish_app "$id" "$name" "$version" "$pkg" "webapp.json" "" "$summary" education "katD2707" \
    "$license" "$source" "false" "$client"
  WEB_URL=""
  cat >&2 <<ENVEOF

Đã đưa Mentor vào kho TUF và danh mục dev. Chạy gateway Mentor (đã áp bản vá) với các biến sau
(client secret nằm trong $secret_file, không commit):
  MENTOR_ENV=dev
  PUBLIC_BASE_URL=$back
  WEB_ORIGIN=$front
  FECO_ISSUER_URL=${HYDRA_PUBLIC:-http://127.0.0.1:4444}/
  FECO_SERVER_URL=${FECO_SERVER_URL:-http://127.0.0.1:8080/}
  FECO_CLIENT_ID=$client
  FECO_CLIENT_SECRET=<nội dung tệp $secret_file>
ENVEOF
}

case "${1:-}" in
  samples) samples ;;
  thesisai) thesisai ;;
  binhai) binhai ;;
  mentor) mentor ;;
  icons) icons ;;
  github) shift; github "$@" ;;
  opensource) opensource ;;
  all) samples; opensource ;;
  *) sed -n '2,21p' "$0"; exit 2 ;;
esac
