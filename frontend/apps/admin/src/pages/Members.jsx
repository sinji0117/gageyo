import { useState } from "react";
import { Modal, Badge, Empty, Loading, ErrorBox, Tabs, useLoad, dateText, dateTimeText, qs } from "@gageyo/ui";
import { useAdmin } from "../context.js";
import { aapi } from "../aapi.js";

const ROLE = { customer: "고객", workshop: "파트너", admin: "운영자" };

/** 회원 관리 (예전에는 '준비 중' 화면만 있었음) */
export default function Members() {
  const { tabParams, toast } = useAdmin();
  const [role, setRole] = useState(tabParams.role || "customer");
  const [q, setQ] = useState("");
  const [term, setTerm] = useState("");
  const { data, loading, error, reload } = useLoad(() => aapi(`/admin/users${qs({ role, q: term, size: 200 })}`), [role, term]);
  const [detail, setDetail] = useState(null);

  async function setStatus(u, status) {
    let reason = null;
    if (status === "suspended") {
      reason = window.prompt(`${u.email} 계정을 정지할까요? 바로 로그아웃되고 로그인할 수 없습니다.\n정지 사유:`, "");
      if (!reason) return;
    }
    try {
      await aapi(`/admin/users/${u.id}`, { method: "PATCH", body: { status, reason } });
      toast(status === "suspended" ? "정지했습니다." : "정지를 풀었습니다.");
      reload();
      if (detail) setDetail(await aapi(`/admin/users/${u.id}`));
    } catch (e) {
      toast(e.message);
    }
  }
  return (
    <>
      <div className="page-title">
        <div>
          <h1>회원 관리</h1>
          <p>목록의 연락처는 가운데를 가립니다. 회원을 누르면 전체 정보를 봅니다 (조회도 운영 목적에만).</p>
        </div>
      </div>
      <Tabs
        tabs={[
          ["customer", "고객"],
          ["workshop", "파트너"],
          ["admin", "운영자"],
          ["", "전체"],
        ]}
        value={role}
        onChange={setRole}
      />
      <form className="search-row" onSubmit={(e) => { e.preventDefault(); setTerm(q); }}>
        <input value={q} onChange={(e) => setQ(e.target.value)} placeholder="이메일·이름·연락처" />
        <button className="gy-button ghost small">검색</button>
      </form>
      <ErrorBox error={error} onRetry={reload} />
      {loading && !data && <Loading />}
      {data?.length === 0 && <Empty>회원이 없습니다.</Empty>}
      {data?.length > 0 && (
        <div className="box gy-table-wrap">
          <table className="gy-table">
            <thead>
              <tr>
                <th>회원</th>
                <th>연락처</th>
                <th>구분</th>
                <th>주문·예약</th>
                <th>가입</th>
                <th>최근 로그인</th>
                <th>상태</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {data.map((u) => (
                <tr key={u.id}>
                  <td>
                    <button type="button" className="link-button" onClick={async () => setDetail(await aapi(`/admin/users/${u.id}`))}>
                      {u.display_name}
                    </button>
                    <small>{u.email}</small>
                  </td>
                  <td>{u.phone || "-"}</td>
                  <td>
                    {ROLE[u.role] || u.role}
                    {u.is_admin && <small>운영자</small>}
                  </td>
                  <td>
                    {u.orders} · {u.bookings}
                  </td>
                  <td>{dateText(u.created_at)}</td>
                  <td>{u.last_login_at ? dateText(u.last_login_at) : "-"}</td>
                  <td>
                    <Badge kind="user" value={u.status} />
                  </td>
                  <td>
                    {u.status === "active" && !u.is_admin && (
                      <button type="button" className="gy-button danger small" onClick={() => setStatus(u, "suspended")}>
                        정지
                      </button>
                    )}
                    {u.status === "suspended" && (
                      <button type="button" className="gy-button ghost small" onClick={() => setStatus(u, "active")}>
                        정지 해제
                      </button>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {detail && (
        <Modal title={detail.display_name} eyebrow={ROLE[detail.role]} onClose={() => setDetail(null)}>
          <dl className="kv">
            <dt>이메일</dt>
            <dd>
              {detail.email} {detail.email_verified_at ? "(인증)" : ""}
            </dd>
            <dt>연락처</dt>
            <dd>{detail.phone || "-"}</dd>
            <dt>상태</dt>
            <dd>
              <Badge kind="user" value={detail.status} />
            </dd>
            <dt>가입</dt>
            <dd>{dateTimeText(detail.created_at)}</dd>
            <dt>최근 로그인</dt>
            <dd>
              {detail.last_login_at ? dateTimeText(detail.last_login_at) : "-"} {detail.last_login_ip && <small>({detail.last_login_ip})</small>}
            </dd>
            <dt>활동</dt>
            <dd>
              주문 {detail.counts.orders} · 예약 {detail.counts.bookings} · 견적 {detail.counts.quotes} · 리뷰 {detail.counts.reviews}
            </dd>
            {detail.workshops.length > 0 && (
              <>
                <dt>업체</dt>
                <dd>{detail.workshops.map((w) => `${w.name}(${w.status})`).join(", ")}</dd>
              </>
            )}
          </dl>
          <div className="gy-actions">
            {detail.status === "active" && !detail.is_admin && (
              <button type="button" className="gy-button danger" onClick={() => setStatus(detail, "suspended")}>
                계정 정지
              </button>
            )}
            {detail.status === "suspended" && (
              <button type="button" className="gy-button" onClick={() => setStatus(detail, "active")}>
                정지 해제
              </button>
            )}
          </div>
        </Modal>
      )}
    </>
  );
}
