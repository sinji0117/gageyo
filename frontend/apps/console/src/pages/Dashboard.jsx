import { useEffect, useState } from "react";
import { money, dateText, dateTimeText, Badge, Empty } from "@gageyo/ui";
import { usePartner } from "../context.js";
import { papi } from "../papi.js";

export default function Dashboard() {
  const { me, go } = usePartner();
  const approved = me.status === "approved";
  const [stats, setStats] = useState(null);
  const [notices, setNotices] = useState([]);
  const [quotes, setQuotes] = useState([]);
  const [bookings, setBookings] = useState([]);
  const [setup, setSetup] = useState({ services: 0, photos: 0 });

  useEffect(() => {
    papi("/partner/notices?limit=5").then(setNotices).catch(() => {});
    Promise.all([papi("/partner/services"), papi("/partner/photos")])
      .then(([s, p]) => setSetup({ services: s.filter((x) => x.is_active).length, photos: p.length }))
      .catch(() => {});
    if (approved) {
      papi("/partner/dashboard").then(setStats).catch(() => {});
      papi("/partner/quotes").then((q) => setQuotes(q.slice(0, 5))).catch(() => {});
      papi("/partner/bookings?when=upcoming&size=6").then(setBookings).catch(() => {});
    }
  }, [approved]);

  const checklist = [
    ["업체 소개·영업시간", Boolean(me.open_time && me.description), "profile"],
    ["업체 사진 (1장 이상)", setup.photos > 0, "profile"],
    ["예약 받을 서비스", setup.services > 0, "services"],
    ["위치(지도) 입력", me.latitude != null, "profile"],
  ];
  const left = checklist.filter(([, ok]) => !ok);

  return (
    <>
      <div className="welcome">
        <div>
          <h1>{me.name}</h1>
          <p>
            {me.district} · {(me.categories || []).join(", ")} · ★ {Number(me.rating || 0).toFixed(1)} ({me.review_count})
          </p>
        </div>
        <button type="button" className="gy-button" onClick={() => go("products")}>
          ＋ 상품 등록
        </button>
      </div>

      {approved && stats && (
        <section className="metrics">
          {[
            ["오늘 예약", `${stats.bookings_today}건`, `확정 대기 ${stats.bookings_requested}건`, "bookings"],
            ["다가오는 확정 예약", `${stats.bookings_upcoming}건`, "예약 일정 보기", "bookings"],
            ["보낼 상품", `${stats.orders_to_ship}건`, "주문·배송", "orders"],
            ["이번 달 판매", money(stats.month_sales), "결제 완료 기준", "settlements"],
            ["30일 견적", `${stats.offers_sent_30d}건`, `선택됨 ${stats.offers_accepted_30d}건`, "offers"],
            ["답글 없는 리뷰", `${stats.reviews_unanswered}개`, "리뷰 관리", "reviews"],
          ].map(([label, value, note, tab]) => (
            <button type="button" key={label} onClick={() => go(tab)}>
              <span>{label}</span>
              <b>{value}</b>
              <small>{note}</small>
            </button>
          ))}
        </section>
      )}

      {left.length > 0 && (
        <section className="panel">
          <div className="panel-title">
            <div>
              <h2>업체 준비하기</h2>
              <p>정보를 채울수록 고객이 더 많이 찾고 예약합니다.</p>
            </div>
          </div>
          <ul className="checklist">
            {checklist.map(([label, ok, tab]) => (
              <li key={label} className={ok ? "done" : ""}>
                <span>{ok ? "✓" : "○"}</span> {label}
                {!ok && (
                  <button type="button" className="link-button" onClick={() => go(tab)}>
                    채우기 →
                  </button>
                )}
              </li>
            ))}
          </ul>
        </section>
      )}

      <div className="grid-2">
        {approved && (
          <section className="panel">
            <div className="panel-title">
              <div>
                <h2>새 견적 요청</h2>
                <p>분야·지역이 맞고 아직 답하지 않은 요청</p>
              </div>
              <button type="button" onClick={() => go("quotes")}>
                전체 보기 →
              </button>
            </div>
            {quotes.length ? (
              <ul className="mini-list">
                {quotes.map((q) => (
                  <li key={q.id}>
                    <b>{q.vehicle}</b>
                    <span>
                      {q.service_category} · {q.district}
                    </span>
                    <small>{dateText(q.created_at)}</small>
                  </li>
                ))}
              </ul>
            ) : (
              <Empty>새 견적 요청이 없습니다.</Empty>
            )}
          </section>
        )}
        {approved && (
          <section className="panel">
            <div className="panel-title">
              <div>
                <h2>다가오는 예약</h2>
              </div>
              <button type="button" onClick={() => go("bookings")}>
                전체 보기 →
              </button>
            </div>
            {bookings.length ? (
              <ul className="mini-list">
                {bookings.map((b) => (
                  <li key={b.id}>
                    <b>{dateTimeText(b.starts_at)}</b>
                    <span>
                      {b.customer_name} · {b.vehicle}
                    </span>
                    <Badge kind="booking" value={b.status} />
                  </li>
                ))}
              </ul>
            ) : (
              <Empty>다가오는 예약이 없습니다.</Empty>
            )}
          </section>
        )}
        <section className="panel">
          <div className="panel-title">
            <div>
              <h2>가게요 공지</h2>
            </div>
            <button type="button" onClick={() => go("support")}>
              공지·문의 →
            </button>
          </div>
          {notices.length ? (
            <ul className="mini-list">
              {notices.map((n) => (
                <li key={n.id}>
                  <b>
                    {n.pinned ? "📌 " : ""}
                    {n.title}
                  </b>
                  <small>{dateText(n.created_at)}</small>
                </li>
              ))}
            </ul>
          ) : (
            <Empty>공지가 없습니다.</Empty>
          )}
        </section>
      </div>
    </>
  );
}
