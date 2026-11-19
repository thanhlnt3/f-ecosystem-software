// Chat của F-Eco (design C3): nhận sự kiện qua Server-Sent Events, tải phần tin mới và chèn vào
// cuối luồng tin; trang danh sách thì tải lại danh sách. Không có JavaScript thì trang vẫn dùng
// được (gửi tin rồi chuyển hướng).
(function () {
  "use strict";
  var thread = document.getElementById("chat-thread");
  var list = document.getElementById("chat-list");
  if (!window.EventSource || !window.fetch || (!thread && !list)) {
    return;
  }
  var conv = thread ? thread.getAttribute("data-conversation") : "";
  var busy = false;
  var again = false;

  function rowId(el) {
    return parseInt(el.getAttribute("data-id"), 10) || 0;
  }

  // Tải lại từ tin có tệp đang quét (để cập nhật trạng thái tệp) hoặc từ tin cuối.
  function since() {
    var last = parseInt(thread.getAttribute("data-last"), 10) || 0;
    var pending = thread.querySelectorAll(".chat-att[data-status=pending]");
    for (var i = 0; i < pending.length; i++) {
      var row = pending[i].closest(".chat-msg");
      if (row && rowId(row) - 1 < last) {
        last = rowId(row) - 1;
      }
    }
    return last;
  }

  function pull() {
    if (busy) {
      again = true;
      return;
    }
    busy = true;
    fetch("/messages/" + conv + "/after/" + since(), { credentials: "same-origin" })
      .then(function (r) {
        return r.ok ? r.text() : "";
      })
      .then(function (html) {
        if (!html.trim()) {
          return;
        }
        var empty = document.getElementById("chat-empty");
        if (empty) {
          empty.parentNode.removeChild(empty);
        }
        var box = document.createElement("div");
        box.innerHTML = html;
        var rows = box.querySelectorAll(".chat-msg");
        var last = parseInt(thread.getAttribute("data-last"), 10) || 0;
        for (var i = 0; i < rows.length; i++) {
          var row = rows[i];
          var old = document.getElementById(row.id);
          if (old) {
            old.parentNode.replaceChild(row, old);
          } else {
            thread.appendChild(row);
          }
          if (rowId(row) > last) {
            last = rowId(row);
          }
        }
        thread.setAttribute("data-last", String(last));
        thread.scrollTop = thread.scrollHeight;
      })
      .catch(function () {})
      .then(function () {
        busy = false;
        if (again) {
          again = false;
          pull();
        }
      });
  }

  var reloadTimer = null;
  var url = "/messages/events" + (conv ? "?viewing=" + encodeURIComponent(conv) : "");
  var events = new EventSource(url);
  events.addEventListener("message", function (e) {
    var data;
    try {
      data = JSON.parse(e.data);
    } catch (err) {
      return;
    }
    if (thread && data.conversation === conv) {
      pull();
    } else if (list && !reloadTimer) {
      reloadTimer = setTimeout(function () {
        window.location.reload();
      }, 500);
    }
  });
  if (thread) {
    thread.scrollTop = thread.scrollHeight;
  }
})();
