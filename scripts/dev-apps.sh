#!/usr/bin/env bash
# Đưa thêm app vào môi trường dev của F-Launcher (thay đổi improve-launcher-experience, task 3.2).
# Cần đã chạy `scripts/dev-launcher.sh setup` (kho TUF, khóa) và Hydra đang chạy.
#
#   scripts/dev-apps.sh samples     # app mẫu dùng F-SDK: Ghi chú (f-notes), Bấm giờ (f-timer)
#   scripts/dev-apps.sh github <owner/repo> <tag-hoặc-commit> --id ID --name TÊN --bin FILE \
#       --license SPDX [--summary MÔ-TẢ] [--category NHÓM] [--developer TÊN] [--terminal] \
#       [--description GIỚI-THIỆU] [--features "A|B|C"] [--website URL] [--notes GHI-CHÚ]
#                                   # tải mã nguồn đúng tag/commit, dựng, đóng gói kèm giấy phép
#                                   # gốc, ký TUF, ghi danh mục (xem docs/github-apps.md)
#   scripts/dev-apps.sh opensource  # ba app mã nguồn mở của danh mục dev: kibi, bottom,
#                                   # eframe template
#   scripts/dev-apps.sh all         # samples + opensource
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

# Nội dung trang chi tiết của app sắp phát hành (đặt trước khi gọi publish_app).
DESCRIPTION="" FEATURES="" WEBSITE="" NOTES=""

# publish_app ID TÊN VERSION THƯ-MỤC-GÓI FILE-CHẠY CLIENT-ID-TUF SUMMARY CATEGORY DEVELOPER LICENSE \
#             SOURCE TERMINAL CATALOG-CLIENT-ID
publish_app() {
  local id="$1" name="$2" version="$3" pkg="$4" entry="$5" tuf_client="$6" summary="$7"
  local category="$8" developer="$9" license="${10}" source_url="${11}" terminal="${12}"
  local catalog_client="${13}"
  local plat zipname zipfile
  plat="$(platform)"
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
  [[ -n "$tuf_client" ]] && args+=(--client-id "$tuf_client")
  [[ -n "$source_url" ]] && args+=(--source "$source_url")
  [[ "$terminal" == "true" ]] && args+=(--terminal true)
  "$TUF" "${args[@]}"
  catalog_entry
}

# Ghi app và phiên bản vào danh mục F-Store (dùng biến cục bộ của publish_app).
catalog_entry() {
  local json="$DEV/catalog-$id.json"
  cat > "$json" <<EOF
{"id":"$id","client_id":"$catalog_client","name":$(json_str "$name"),
 "developer":$(json_str "$developer"),"developer_verified":false,"first_party":false,
 "summary":$(json_str "$summary"),
 "description":$(json_str "${DESCRIPTION:-$summary}${source_url:+ Mã nguồn: $source_url}"),
 "category":"$category","price_vnd":0,"license":$(json_str "$license"),"screenshots":[]}
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
  DESCRIPTION="" FEATURES="" WEBSITE="" NOTES=""
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
  github ilai-deutel/kibi v0.3.3 --id kibi --name "kibi" --bin kibi --license "MIT OR Apache-2.0" \
    --summary "Trình soạn thảo văn bản siêu nhỏ chạy trong terminal (dưới 1.024 dòng mã)." \
    --category tools --developer "Ilaï Deutel" --terminal \
    --description "kibi là trình soạn thảo văn bản trong terminal, viết bằng Rust với chưa tới 1.024 dòng mã, lấy cảm hứng từ kilo. Bản trong F-Eco dựng từ mã nguồn gốc tại tag v0.3.3, không sửa đổi." \
    --features "Tô màu cú pháp cho hơn 30 ngôn ngữ|Tìm kiếm tăng dần|Sao chép, cắt, dán, nhân đôi dòng|Chạy lệnh ngoài và dán kết quả vào văn bản|Cấu hình qua tệp config.ini|Chạy trên Windows, macOS, Linux" \
    --notes "Bản 0.3.3 của dự án gốc: thêm Ctrl+/ để chú thích hoặc bỏ chú thích dòng; bật bảo vệ ngăn xếp cho bản phát hành. Chi tiết trong CHANGELOG.md của kho ilai-deutel/kibi."
  github ClementTsang/bottom 0.15.0 --id bottom --name "bottom" --bin btm --license "MIT" \
    --summary "Theo dõi CPU, bộ nhớ, mạng, ổ đĩa và tiến trình ngay trong terminal." \
    --category system --developer "Clement Tsang" --terminal --website "https://bottom.pages.dev/stable" \
    --description "bottom (lệnh btm) là công cụ theo dõi hệ thống đa nền tảng trong terminal, lấy cảm hứng từ gtop, gotop và htop. Bản trong F-Eco dựng từ mã nguồn gốc tại tag 0.15.0, không sửa đổi." \
    --features "Biểu đồ CPU, bộ nhớ, mạng theo thời gian|Danh sách tiến trình: lọc, sắp xếp, dừng tiến trình|Dung lượng và tốc độ đọc ghi ổ đĩa|Nhiệt độ cảm biến và pin|Giao diện tùy chỉnh bằng tệp cấu hình|Điều khiển bằng bàn phím và chuột" \
    --notes "Bản 0.15.0 của dự án gốc: hỗ trợ GPU Intel trên Linux, thanh dạng khối, đơn vị dung lượng nhị phân cho ổ đĩa, cột swap của tiến trình trên Linux; bật thanh cuộn mặc định; sửa lỗi khởi động trên Windows 7 và 8. Chi tiết trong CHANGELOG.md của kho ClementTsang/bottom."
  github emilk/eframe_template f108546a2b9f2fc6a82543ceae1625a3893305cb --id eframe-template \
    --name "eframe template" --bin eframe_template --license "MIT OR Apache-2.0" \
    --summary "App cửa sổ mẫu viết bằng egui/eframe, điểm khởi đầu cho app đồ họa bằng Rust." \
    --category development --developer "Emil Ernerfeldt" --website "https://www.egui.rs" \
    --description "eframe template là app mẫu của tác giả egui, dùng làm điểm khởi đầu cho app đồ họa viết bằng Rust chạy trên máy tính và trên web. Bản trong F-Eco dựng từ commit f108546, không sửa đổi." \
    --features "Cửa sổ đồ họa egui với chủ đề sáng và tối|Dựng cho máy tính và cho web (WebAssembly)|Mã ngắn, dễ sao chép làm app mới" \
    --notes "Bản tại commit f108546 của dự án gốc, dùng egui và eframe 0.36."
}

case "${1:-}" in
  samples) samples ;;
  github) shift; github "$@" ;;
  opensource) opensource ;;
  all) samples; opensource ;;
  *) sed -n '2,16p' "$0"; exit 2 ;;
esac
