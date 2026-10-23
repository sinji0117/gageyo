import { useCallback, useEffect, useRef, useState } from "react";
import { uploadImage } from "./api.js";
import { statusLabel, statusTone } from "./format.js";

/** 잠깐 보이는 알림. const [toast, show] = useToast(); ... {toast} */
export function useToast() {
  const [message, setMessage] = useState("");
  const timer = useRef(0);
  const show = useCallback((text) => {
    setMessage(text);
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setMessage(""), 3200);
  }, []);
  const node = message ? (
    <div className="gy-toast" role="status">
      {message}
    </div>
  ) : null;
  return [node, show];
}

/** API 불러오기: const { data, loading, error, reload } = useLoad(() => api("/x"), [deps]) */
export function useLoad(loader, deps = []) {
  const [state, setState] = useState({ data: null, loading: true, error: "" });
  const seq = useRef(0);
  const reload = useCallback(async () => {
    const n = ++seq.current;
    setState((s) => ({ ...s, loading: true, error: "" }));
    try {
      const data = await loader();
      if (n === seq.current) setState({ data, loading: false, error: "" });
      return data;
    } catch (e) {
      if (n === seq.current) setState((s) => ({ ...s, loading: false, error: e.message || "불러오지 못했습니다." }));
      return null;
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);
  useEffect(() => {
    reload();
  }, [reload]);
  return { ...state, reload, setData: (d) => setState((s) => ({ ...s, data: typeof d === "function" ? d(s.data) : d })) };
}

export function Modal({ title, eyebrow, onClose, children, wide = false, footer = null }) {
  useEffect(() => {
    const onKey = (e) => e.key === "Escape" && onClose?.();
    window.addEventListener("keydown", onKey);
    const prev = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      window.removeEventListener("keydown", onKey);
      document.body.style.overflow = prev;
    };
  }, [onClose]);
  return (
    <div className="gy-overlay" onMouseDown={(e) => e.target === e.currentTarget && onClose?.()}>
      <section className={`gy-modal ${wide ? "wide" : ""}`} role="dialog" aria-modal="true" aria-label={title}>
        <header className="gy-modal-head">
          <div>
            {eyebrow && <span className="gy-eyebrow">{eyebrow}</span>}
            <h2>{title}</h2>
          </div>
          <button type="button" className="gy-close" onClick={onClose} aria-label="닫기">
            ×
          </button>
        </header>
        <div className="gy-modal-body">{children}</div>
        {footer && <footer className="gy-modal-foot">{footer}</footer>}
      </section>
    </div>
  );
}

export function Badge({ kind, value, children }) {
  return <span className={`gy-badge ${statusTone(value)}`}>{children || statusLabel(kind, value)}</span>;
}

export function Empty({ children, action = null }) {
  return (
    <div className="gy-empty">
      <p>{children}</p>
      {action}
    </div>
  );
}

export function Loading({ text = "불러오는 중…" }) {
  return <div className="gy-loading">{text}</div>;
}

export function ErrorBox({ error, onRetry }) {
  if (!error) return null;
  return (
    <div className="gy-error" role="alert">
      <span>{error}</span>
      {onRetry && (
        <button type="button" onClick={onRetry}>
          다시 시도
        </button>
      )}
    </div>
  );
}

export function Field({ label, hint, children, required = false }) {
  return (
    <label className="gy-field">
      <span>
        {label}
        {required && <i aria-hidden="true"> *</i>}
      </span>
      {children}
      {hint && <small>{hint}</small>}
    </label>
  );
}

/** 이미지 하나 올리기 (업체 사진·상품 이미지) */
export function ImageInput({ value, onChange, onError, label = "이미지" }) {
  const [busy, setBusy] = useState(false);
  async function pick(e) {
    const file = e.target.files?.[0];
    e.target.value = "";
    if (!file) return;
    setBusy(true);
    try {
      const info = await uploadImage(file);
      onChange(info.url);
    } catch (err) {
      onError?.(err.message);
    } finally {
      setBusy(false);
    }
  }
  return (
    <div className="gy-image-input">
      <div className="gy-image-preview" style={value ? { backgroundImage: `url(${value})` } : undefined}>
        {!value && <span>{label}</span>}
      </div>
      <div className="gy-image-actions">
        <label className="gy-button ghost">
          {busy ? "올리는 중…" : value ? "바꾸기" : "올리기"}
          <input type="file" accept="image/jpeg,image/png,image/gif,image/webp" onChange={pick} disabled={busy} hidden />
        </label>
        {value && (
          <button type="button" className="gy-button ghost" onClick={() => onChange("")}>
            빼기
          </button>
        )}
      </div>
    </div>
  );
}

export function Tabs({ tabs, value, onChange }) {
  return (
    <div className="gy-tabs" role="tablist">
      {tabs.map(([id, label, count]) => (
        <button key={id} type="button" role="tab" aria-selected={value === id} className={value === id ? "on" : ""} onClick={() => onChange(id)}>
          {label}
          {count ? <i>{count}</i> : null}
        </button>
      ))}
    </div>
  );
}

/** 이메일 인증 코드 받기 버튼 (다시 받기 60초 표시) */
export function CodeButton({ onSend }) {
  const [left, setLeft] = useState(0);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    if (left <= 0) return undefined;
    const t = window.setTimeout(() => setLeft(left - 1), 1000);
    return () => window.clearTimeout(t);
  }, [left]);
  async function click() {
    setBusy(true);
    try {
      await onSend();
      setLeft(60);
    } finally {
      setBusy(false);
    }
  }
  return (
    <button type="button" className="gy-button ghost" disabled={busy || left > 0} onClick={click}>
      {busy ? "보내는 중…" : left > 0 ? `다시 받기 (${left}초)` : "인증 코드 받기"}
    </button>
  );
}
