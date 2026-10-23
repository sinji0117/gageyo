// 토스페이먼츠 결제창 (SDK v2). 키가 설정된 서버에서만 씁니다.
// 결제가 끝나면 successUrl 로 돌아오고, App 이 /api/orders/{id}/payments/toss 로 승인을 요청합니다.

const SDK_URL = "https://js.tosspayments.com/v2/standard";
let loading = null;

function loadSdk() {
  if (window.TossPayments) return Promise.resolve(window.TossPayments);
  if (!loading) {
    loading = new Promise((resolve, reject) => {
      const s = document.createElement("script");
      s.src = SDK_URL;
      s.async = true;
      s.onload = () => (window.TossPayments ? resolve(window.TossPayments) : reject(new Error("결제창을 불러오지 못했습니다.")));
      s.onerror = () => {
        loading = null;
        reject(new Error("결제창을 불러오지 못했습니다. 잠시 후 다시 시도해 주세요."));
      };
      document.head.appendChild(s);
    });
  }
  return loading;
}

/**
 * pay = 서버 주문 응답의 toss 값 { client_key, order_id, order_name, amount, customer_name, customer_email, customer_key }
 */
export async function requestTossPayment(pay) {
  const TossPayments = await loadSdk();
  const tp = TossPayments(pay.client_key);
  const payment = tp.payment({ customerKey: pay.customer_key || TossPayments.ANONYMOUS });
  const base = `${window.location.origin}${window.location.pathname}`;
  await payment.requestPayment({
    method: "CARD",
    amount: { currency: "KRW", value: pay.amount },
    orderId: pay.order_id,
    orderName: pay.order_name,
    customerName: pay.customer_name || undefined,
    customerEmail: pay.customer_email || undefined,
    successUrl: `${base}?pay=success`,
    failUrl: `${base}?pay=fail`,
  });
}

/** 비회원 결제 뒤 주문 상세를 보여 주기 위해 연락처를 잠깐 기억 (탭을 닫으면 사라짐) */
export function rememberOrderPhone(orderId, phone) {
  try {
    window.sessionStorage.setItem(`gageyo_order_phone_${orderId}`, phone);
  } catch {
    /* 무시 */
  }
}

export function recallOrderPhone(orderId) {
  try {
    return window.sessionStorage.getItem(`gageyo_order_phone_${orderId}`) || "";
  } catch {
    return "";
  }
}
