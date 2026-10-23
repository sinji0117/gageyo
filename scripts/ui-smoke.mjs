// 화면 점검: 빌드한 네 화면을 실제 API 에 붙여 브라우저로 열고, 주요 화면을 찍고, 자바스크립트 오류를 모읍니다.
// scripts/ui-smoke.sh 가 API·화면 서버와 검사용 데이터를 준비한 뒤 부릅니다.
//
// 환경 변수: OUT(스크린샷 폴더), CHROME(브라우저 실행 파일), CUSTOMER_EMAIL, PARTNER_EMAIL, PASSWORD, ADMIN_KEY, QUOTE_ID
import { mkdirSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";

// ESM import 는 NODE_PATH 를 보지 않으므로 require 로 불러옵니다 (CI 는 /tmp 에 따로 설치).
const require = createRequire(import.meta.url);
const { chromium } = require("playwright-core");

const OUT = process.env.OUT || "/tmp/ui-shots";
const SITE = "http://127.0.0.1:4173";
const CONSOLE = "http://127.0.0.1:4174";
const ADMIN = "http://127.0.0.1:4175";
const WWW = "http://127.0.0.1:4176";
mkdirSync(OUT, { recursive: true });

const errors = [];
const steps = [];
const browser = await chromium.launch({ executablePath: process.env.CHROME || "/usr/bin/google-chrome", args: ["--no-sandbox"] });

async function newPage(viewport = { width: 1280, height: 900 }) {
  const ctx = await browser.newContext({ viewport, locale: "ko-KR", timezoneId: "Asia/Seoul" });
  const page = await ctx.newPage();
  page.on("pageerror", (e) => errors.push(`[pageerror] ${page.url()} ${e.message}`));
  page.on("console", (m) => {
    if (m.type() === "error" && !m.text().includes("Failed to load resource")) errors.push(`[console] ${page.url()} ${m.text()}`);
  });
  return page;
}

async function step(name, fn) {
  try {
    await fn();
    steps.push(`ok   ${name}`);
  } catch (e) {
    steps.push(`FAIL ${name}: ${e.message.split("\n")[0]}`);
  }
}

const shot = (page, name, full = true) => page.screenshot({ path: `${OUT}/${name}.png`, fullPage: full });

// ---------------- 고객 화면 ----------------
const site = await newPage();
await step("고객 첫 화면", async () => {
  await site.goto(SITE, { waitUntil: "networkidle" });
  await site.waitForSelector(".shop-card", { timeout: 15000 });
  await shot(site, "01-site-home");
});
await step("업체 상세", async () => {
  await site.locator(".shop-card .card-hit").first().click();
  await site.waitForSelector(".workshop-detail", { timeout: 10000 });
  await site.waitForTimeout(400);
  await shot(site, "02-site-workshop", false);
});
await step("예약 창", async () => {
  await site.getByRole("button", { name: "방문 예약하기" }).click();
  await site.waitForSelector("text=예약 신청하기", { timeout: 10000 });
  await shot(site, "03-site-booking", false);
  await site.keyboard.press("Escape");
});
await step("상품·장바구니·주문서", async () => {
  await site.getByRole("button", { name: "자동차 용품" }).first().click();
  await site.waitForSelector(".product-card", { timeout: 10000 });
  await shot(site, "04-site-products");
  await site.locator(".product-price button").first().click();
  await site.locator(".cart-button").click();
  await site.waitForSelector(".cart-list", { timeout: 5000 });
  await shot(site, "05-site-cart", false);
  await site.getByRole("button", { name: "주문서 작성" }).click();
  await site.waitForSelector("text=결제 방법", { timeout: 5000 });
  await shot(site, "06-site-checkout", false);
  await site.keyboard.press("Escape");
});
await step("견적 요청 창", async () => {
  await site.getByRole("button", { name: "견적 요청" }).first().click();
  await site.waitForSelector("text=무료 견적 요청하기", { timeout: 5000 });
  await shot(site, "07-site-quote", false);
  await site.keyboard.press("Escape");
});
await step("고객 로그인·마이페이지", async () => {
  await site.getByRole("button", { name: "로그인" }).first().click();
  await site.fill("input[type=email]", process.env.CUSTOMER_EMAIL);
  await site.fill("input[type=password]", process.env.PASSWORD);
  await site.locator(".gy-modal button.gy-button.block").click();
  await site.waitForSelector(".top-actions >> text=님", { timeout: 10000 });
  await site.locator(".top-actions .text-button").first().click();
  await site.waitForSelector(".mypage", { timeout: 10000 });
  await shot(site, "08-site-mypage");
  await site.getByRole("tab", { name: "견적" }).click();
  await site.waitForTimeout(800);
  await shot(site, "09-site-my-quotes");
});
await step("받은 견적 비교", async () => {
  await site.goto(`${SITE}/?quote=${process.env.QUOTE_ID}`, { waitUntil: "networkidle" });
  await site.waitForSelector(".offer-list", { timeout: 10000 });
  await shot(site, "10-site-quote-view", false);
});
const mobile = await newPage({ width: 390, height: 844 });
await step("고객 화면 휴대폰", async () => {
  await mobile.goto(SITE, { waitUntil: "networkidle" });
  await mobile.waitForSelector(".shop-card", { timeout: 15000 });
  await shot(mobile, "11-site-mobile");
  await mobile.locator(".menu-button").click();
  await shot(mobile, "12-site-mobile-menu", false);
});

// ---------------- 파트너 센터 ----------------
const partner = await newPage();
await step("파트너 입점 신청 화면", async () => {
  await partner.goto(CONSOLE, { waitUntil: "networkidle" });
  await partner.getByRole("button", { name: "입점 신청하기" }).click();
  await partner.waitForSelector("text=가게요 입점 신청");
  await shot(partner, "20-console-apply");
  await partner.getByRole("button", { name: "← 로그인" }).click();
});
await step("파트너 로그인·대시보드", async () => {
  await partner.fill("input[type=email]", process.env.PARTNER_EMAIL);
  await partner.fill("input[type=password]", process.env.PASSWORD);
  await partner.getByRole("button", { name: "로그인" }).click();
  await partner.waitForSelector(".layout", { timeout: 10000 });
  await partner.waitForTimeout(800);
  await shot(partner, "21-console-dashboard");
});
for (const [tab, label] of [
  ["quotes", "견적 요청"],
  ["bookings", "예약 일정"],
  ["products", "상품"],
  ["orders", "주문·배송"],
  ["profile", "업체 정보"],
  ["settlements", "정산"],
]) {
  await step(`파트너 ${label}`, async () => {
    await partner.locator("aside nav button", { hasText: label }).click();
    await partner.waitForTimeout(900);
    await shot(partner, `22-console-${tab}`);
  });
}
await step("파트너 견적 보내기 창", async () => {
  await partner.locator("aside nav button", { hasText: "견적 요청" }).click();
  const btn = partner.getByRole("button", { name: "견적 보내기" }).first();
  if (await btn.count()) {
    await btn.click();
    await partner.waitForSelector("text=견적 금액");
    await shot(partner, "23-console-offer-form", false);
    await partner.keyboard.press("Escape");
  }
});

// ---------------- 운영자 ----------------
const admin = await newPage();
await step("운영자 로그인 화면", async () => {
  await admin.goto(ADMIN, { waitUntil: "networkidle" });
  await shot(admin, "30-admin-login", false);
});
await step("운영자 API 키 접속·대시보드", async () => {
  await admin.getByRole("button", { name: "API 키로 접속" }).click();
  await admin.fill("input[type=password]", process.env.ADMIN_KEY);
  await admin.getByRole("button", { name: "접속" }).click();
  await admin.waitForSelector(".metric-grid", { timeout: 10000 });
  await shot(admin, "31-admin-dashboard");
});
for (const [tab, label] of [
  ["workshops", "업체 관리"],
  ["members", "회원 관리"],
  ["orders", "주문"],
  ["inquiries", "문의·신고"],
  ["settings", "결제·운영 설정"],
  ["categories", "카테고리"],
  ["audit", "작업 기록"],
]) {
  await step(`운영자 ${label}`, async () => {
    await admin.locator("aside nav button", { hasText: label }).first().click();
    await admin.waitForTimeout(900);
    await shot(admin, `32-admin-${tab}`);
  });
}
await step("운영자 업체 상세", async () => {
  await admin.locator("aside nav button", { hasText: "업체 관리" }).first().click();
  await admin.getByRole("tab", { name: "승인" }).click();
  await admin.waitForTimeout(600);
  await admin.locator("tr.clickable").first().click();
  await admin.waitForSelector(".detail-actions", { timeout: 5000 });
  await shot(admin, "33-admin-workshop-detail", false);
});

// ---------------- 소개 ----------------
const www = await newPage();
await step("소개 페이지", async () => {
  await www.goto(WWW, { waitUntil: "networkidle" });
  await shot(www, "40-www");
});

await browser.close();
writeFileSync(`${OUT}/report.txt`, [...steps, "", "오류:", ...(errors.length ? errors : ["(없음)"])].join("\n"));
console.log(steps.join("\n"));
console.log(errors.length ? `\n자바스크립트 오류 ${errors.length}개:\n${errors.join("\n")}` : "\n자바스크립트 오류 없음");
process.exit(steps.some((s) => s.startsWith("FAIL")) || errors.length ? 1 : 0);
