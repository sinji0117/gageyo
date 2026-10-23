import { useEffect, useState } from "react";
import { api, dateText } from "@gageyo/ui";
import { useApp } from "../context.js";

export default function Footer() {
  const { config, open } = useApp();
  const [notices, setNotices] = useState([]);
  const [shown, setShown] = useState(null);
  useEffect(() => {
    api("/notices?limit=3").then(setNotices).catch(() => setNotices([]));
  }, []);
  const support = config?.support || {};
  return (
    <footer className="site-footer">
      {notices.length > 0 && (
        <div className="notice-strip">
          <b>공지</b>
          {notices.map((n) => (
            <button type="button" key={n.id} onClick={() => setShown(shown?.id === n.id ? null : n)}>
              {n.pinned ? "📌 " : ""}
              {n.title} <small>{dateText(n.created_at)}</small>
            </button>
          ))}
        </div>
      )}
      {shown && (
        <article className="notice-body">
          <h3>{shown.title}</h3>
          <p>{shown.body}</p>
        </article>
      )}
      <div className="footer-grid">
        <div>
          <b className="footer-brand">✳ 가게요</b>
          <p>우리 동네 자동차 관리 플랫폼 · 정비소 찾기 · 견적 비교 · 예약 · 용품 구매</p>
        </div>
        <div>
          <b>고객센터</b>
          <p>
            {support.phone || "전화 상담 준비 중"}
            {support.email && (
              <>
                <br />
                {support.email}
              </>
            )}
          </p>
          <button type="button" className="link-button" onClick={() => open({ type: "support" })}>
            1:1 문의하기 →
          </button>
        </div>
        <div>
          <b>사업자 정보</b>
          <p className="pre">{support.company_info || "가게요"}</p>
        </div>
      </div>
    </footer>
  );
}
