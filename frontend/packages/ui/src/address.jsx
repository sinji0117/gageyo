// 주소 입력: 다음(카카오) 우편번호 서비스로 도로명 주소를 찾아 채우고, 상세 주소는 이어서 직접 입력합니다.
// https://postcode.map.daum.net/guide — 키 없이 무료. 스크립트는 '주소 검색'을 처음 누를 때 불러옵니다.
// 검색 창을 못 열면(차단·오프라인) 직접 입력할 수 있게 칸은 그대로 둡니다. (bandiuri address.jsx 를 옮김)
import { useEffect, useRef, useState } from "react";

const SCRIPT = "https://t1.daumcdn.net/mapjsapi/bundle/postcode/prod/postcode.v2.js";
let loading = null;

export function loadPostcode() {
  if (typeof window === "undefined") return Promise.reject(new Error("no window"));
  if (window.daum?.Postcode) return Promise.resolve(window.daum);
  if (!loading) {
    loading = new Promise((resolve, reject) => {
      const s = document.createElement("script");
      s.src = SCRIPT;
      s.async = true;
      s.onload = () => (window.daum?.Postcode ? resolve(window.daum) : reject(new Error("postcode")));
      s.onerror = () => {
        loading = null;
        reject(new Error("postcode"));
      };
      document.head.appendChild(s);
    });
  }
  return loading;
}

/** 다음 우편번호 결과 → { zipcode, address, sido, sigungu } */
export function fromPostcode(data) {
  const base = data.userSelectedType === "J" ? data.jibunAddress || data.autoJibunAddress : data.roadAddress || data.autoRoadAddress;
  // 도로명 주소면 (법정동, 건물명) 참고 항목을 붙임 — 다음 가이드 예제와 같은 방식
  let extra = "";
  if (data.userSelectedType !== "J") {
    if (data.bname && /[동로가]$/.test(data.bname)) extra += data.bname;
    if (data.buildingName && data.apartment === "Y") extra += (extra ? ", " : "") + data.buildingName;
  }
  return {
    zipcode: data.zonecode || "",
    address: extra ? `${base} (${extra})` : base,
    sido: data.sido || "",
    sigungu: data.sigungu || "",
  };
}

/**
 * 주소 한 칸 + '주소 검색' 버튼. 고르면 `기본 주소 ` 를 채우고 커서를 끝으로 옮겨 상세 주소를 이어 쓰게 합니다.
 * onPick(r) 으로 시·구 같은 값도 받을 수 있습니다 (업체 지역 자동 채우기).
 */
export function AddressInput({ value, onChange, onPick, required = false, maxLength = 255, placeholder = "주소 검색 후 상세 주소를 이어서 입력", ...rest }) {
  const [open, setOpen] = useState(false);
  const [failed, setFailed] = useState(false);
  const input = useRef(null);
  return (
    <>
      <span className="gy-address">
        <input
          ref={input}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          required={required}
          maxLength={maxLength}
          placeholder={placeholder}
          {...rest}
        />
        <button
          type="button"
          className="gy-button ghost small"
          onClick={() => {
            setFailed(false);
            setOpen(true);
          }}
        >
          주소 검색
        </button>
      </span>
      {failed && <small className="gy-address-fail">주소 검색을 열 수 없습니다. 주소를 직접 입력해 주세요.</small>}
      {open && (
        <PostcodeDialog
          onClose={() => setOpen(false)}
          onFail={() => {
            setOpen(false);
            setFailed(true);
          }}
          onPick={(r) => {
            setOpen(false);
            const next = `${r.address} `.slice(0, maxLength);
            onChange(next);
            onPick?.(r);
            setTimeout(() => {
              const el = input.current;
              if (el) {
                el.focus();
                el.setSelectionRange(next.length, next.length);
              }
            }, 0);
          }}
        />
      )}
    </>
  );
}

function PostcodeDialog({ onPick, onClose, onFail }) {
  const box = useRef(null);
  useEffect(() => {
    let alive = true;
    loadPostcode()
      .then((daum) => {
        if (!alive || !box.current) return;
        new daum.Postcode({
          oncomplete: (data) => onPick(fromPostcode(data)),
          width: "100%",
          height: "100%",
          maxSuggestItems: 5,
        }).embed(box.current, { autoClose: false });
      })
      .catch(() => alive && onFail());
    // 바깥 대화 상자(주문서 등)까지 닫히지 않도록 Esc 를 여기서 먼저 받음
    const esc = (e) => {
      if (e.key !== "Escape") return;
      e.stopPropagation();
      onClose();
    };
    window.addEventListener("keydown", esc, true);
    return () => {
      alive = false;
      window.removeEventListener("keydown", esc, true);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  return (
    <div className="gy-overlay gy-postcode-overlay" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <section className="gy-modal gy-postcode" role="dialog" aria-modal="true" aria-label="주소 검색">
        <header className="gy-modal-head">
          <h2>주소 검색</h2>
          <button type="button" className="gy-close" onClick={onClose} aria-label="닫기">
            ×
          </button>
        </header>
        <div ref={box} className="gy-postcode-box" />
      </section>
    </div>
  );
}
