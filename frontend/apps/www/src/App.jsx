import { useEffect, useState } from "react";
import { api } from "@gageyo/ui";

const APP = "https://app.gageyo.com";
const PARTNER = "https://partner.gageyo.com";

const FEATURES = [
  ["01", "동네 업체 찾기", "정비·세차·썬팅·튜닝·랩핑·전기차 충전기까지. 지역·분야·거리로 찾고, 영업시간과 실제 이용 후기를 확인하세요."],
  ["02", "견적 한 번에 비교", "요청서 한 장이면 분야와 지역이 맞는 업체들이 견적을 보냅니다. 금액과 작업 내용을 비교해 고르세요."],
  ["03", "방문 예약", "서비스와 시간을 골라 예약을 신청하면 업체가 확정합니다. 영업시간·휴무일에 맞는 시간만 고를 수 있어요."],
  ["04", "용품 구매 · 방문 장착", "업체가 파는 자동차 용품을 택배로 받거나, 그 업체에 방문해 장착 예약까지 한 번에."],
  ["05", "믿을 수 있는 후기", "작업이 완료된 예약에만 후기를 남길 수 있어 실제 이용자의 평가만 쌓입니다."],
];

const FAQ = [
  ["견적 요청은 무료인가요?", "네. 견적 요청과 비교는 무료이고, 고른 업체에 예약할지는 직접 정합니다. 연락처는 고객이 고른 업체에만 보입니다."],
  ["회원가입 없이도 쓸 수 있나요?", "견적 요청·예약·주문은 비회원도 됩니다. 요청·예약·주문 번호와 연락처로 조회할 수 있어요. 가입하면 마이페이지에서 한 번에 봅니다."],
  ["업체는 어떻게 입점하나요?", "파트너 센터에서 사업자 정보와 함께 입점을 신청하면, 운영자가 확인 후 승인합니다. 승인되면 고객 화면에 업체가 보이고 견적 요청을 받습니다."],
  ["결제는 어떻게 하나요?", "용품은 카드·간편결제(토스페이먼츠) 또는 무통장입금으로 결제합니다. 정비 작업 비용은 방문해서 업체에 직접 결제합니다."],
];

export default function App() {
  const [stats, setStats] = useState(null);
  const [open, setOpen] = useState(0);
  useEffect(() => {
    api("/stats").then(setStats).catch(() => setStats(null));
  }, []);
  return (
    <main>
      <nav>
        <a className="logo" href="/">
          ✳ 가게요
        </a>
        <a href="#features">서비스 소개</a>
        <a href="#partner">파트너 입점</a>
        <a href="#faq">자주 묻는 질문</a>
        <a className="nav-cta" href={APP}>
          업체 찾기 ↗
        </a>
      </nav>
      <section className="hero">
        <span>YOUR NEIGHBORHOOD CAR CARE</span>
        <h1>
          자동차 관리가
          <br />
          필요한 순간, <i>가게요</i>
        </h1>
        <p>내 주변 믿을 수 있는 자동차 업체를 찾고, 견적을 비교하고, 예약과 용품 구매까지 한곳에서.</p>
        <div className="hero-cta">
          <a className="cta" href={APP}>
            업체 찾아보기 ↗
          </a>
          <a className="cta ghost" href={`${APP}/?request=quote`}>
            무료 견적 요청
          </a>
        </div>
        {stats && stats.workshops > 0 && (
          <div className="hero-stats">
            <span>
              <b>{stats.workshops.toLocaleString("ko-KR")}</b> 파트너 업체
            </span>
            {stats.reviews > 0 && (
              <span>
                <b>{stats.reviews.toLocaleString("ko-KR")}</b> 이용 후기
              </span>
            )}
            {stats.quotes > 0 && (
              <span>
                <b>{stats.quotes.toLocaleString("ko-KR")}</b> 견적 요청
              </span>
            )}
          </div>
        )}
      </section>

      <section id="features" className="features">
        <span>ONE PLACE FOR YOUR CAR</span>
        <h2>
          필요한 자동차 케어,
          <br />
          한곳에서 편하게.
        </h2>
        <div>
          {FEATURES.map(([n, title, body]) => (
            <article key={n}>
              <b>{n}</b>
              <h3>{title}</h3>
              <p>{body}</p>
            </article>
          ))}
        </div>
      </section>

      <section className="steps">
        <h2>이렇게 이용해요</h2>
        <ol>
          <li>
            <b>요청서 작성</b>
            <p>차량과 원하는 작업, 지역을 적어요.</p>
          </li>
          <li>
            <b>견적 비교</b>
            <p>업체들이 보낸 금액·작업 내용·평점을 비교해요.</p>
          </li>
          <li>
            <b>선택하고 예약</b>
            <p>마음에 드는 견적을 고르고 방문 시간을 정해요.</p>
          </li>
          <li>
            <b>방문 · 후기</b>
            <p>작업이 끝나면 후기로 다른 운전자를 도와요.</p>
          </li>
        </ol>
      </section>

      <section id="partner" className="partner">
        <div>
          <span>FOR PARTNERS</span>
          <h2>자동차 업체 사장님이라면</h2>
          <p>
            가게요 파트너가 되면 우리 동네 운전자의 견적 요청을 받고, 예약 일정과 용품 판매·배송, 리뷰 답글, 월 정산까지 파트너 센터 한곳에서 관리할 수 있습니다.
          </p>
          <ul>
            <li>분야·지역이 맞는 견적 요청 알림</li>
            <li>영업시간·휴무일에 맞춘 예약 접수</li>
            <li>용품 판매 · 방문 장착 · 택배 발송 처리</li>
            <li>카드·간편결제 대금 월 정산</li>
          </ul>
        </div>
        <a className="cta" href={`${PARTNER}/`}>
          입점 신청하기 ↗
        </a>
      </section>

      <section id="faq" className="faq">
        <h2>자주 묻는 질문</h2>
        {FAQ.map(([q, a], i) => (
          <div key={q} className={open === i ? "open" : ""}>
            <button type="button" onClick={() => setOpen(open === i ? -1 : i)} aria-expanded={open === i}>
              {q}
              <span aria-hidden="true">{open === i ? "−" : "+"}</span>
            </button>
            {open === i && <p>{a}</p>}
          </div>
        ))}
      </section>

      <footer>
        가게요 · 자동차 파트너와 운전자를 연결합니다
        <span>
          <a href={APP}>고객 서비스</a> · <a href={PARTNER}>파트너 센터</a> · <a href={`${APP}/?request=support`}>고객센터</a>
        </span>
      </footer>
    </main>
  );
}
