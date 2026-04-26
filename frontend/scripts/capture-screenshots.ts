import { chromium, type Page } from "@playwright/test";
import * as fs from "node:fs/promises";
import * as path from "node:path";

const VIEWPORTS = [
  { w: 375, h: 812, name: "iphone-se" },
  { w: 414, h: 896, name: "iphone-pro-max" },
  { w: 768, h: 1024, name: "ipad-portrait" },
  { w: 1024, h: 768, name: "ipad-landscape" },
  { w: 1280, h: 800, name: "laptop" },
  { w: 1920, h: 1080, name: "desktop" },
];

const TABS = ["claude", "usage", "config", "token"];
const USAGE_SUBTABS = ["overview", "byCookie", "graveyard"];

const FRONTEND_URL = process.env.FRONTEND_URL ?? "http://localhost:8484";
const OUT_DIR = path.resolve("../screenshots");
const ADMIN_TOKEN = process.env.ADMIN_TOKEN ?? "";

async function login(page: Page): Promise<boolean> {
  const passwordInput = page.locator('input[type="password"]').first();
  if (
    await passwordInput.isVisible({ timeout: 2000 }).catch(() => false)
  ) {
    if (!ADMIN_TOKEN) return false;
    await passwordInput.fill(ADMIN_TOKEN);
    await page.locator('button[type="submit"]').first().click();
    await page.waitForTimeout(800);
  }
  return true;
}

async function clickTab(page: Page, tabId: string) {
  // Try multiple selector strategies — match what the project uses.
  // The frontend uses i18n labels so we try data-* attrs first then text fallbacks.
  const candidates = [
    `[data-tab="${tabId}"]`,
    `[role="tab"][data-tab-id="${tabId}"]`,
    `button[data-tab-id="${tabId}"]`,
    `button:has-text("${tabId}")`,
    `[role="tab"]:has-text("${tabId}")`,
  ];
  for (const sel of candidates) {
    const el = page.locator(sel).first();
    if (await el.isVisible({ timeout: 200 }).catch(() => false)) {
      await el.click();
      await page.waitForTimeout(400);
      return true;
    }
  }
  return false;
}

async function clickButtonByText(page: Page, text: string) {
  const candidates = [
    `button:has-text("${text}")`,
    `[role="tab"]:has-text("${text}")`,
  ];
  for (const sel of candidates) {
    const el = page.locator(sel).first();
    if (await el.isVisible({ timeout: 200 }).catch(() => false)) {
      await el.click();
      await page.waitForTimeout(400);
      return true;
    }
  }
  return false;
}

async function captureForViewport(
  vp: { w: number; h: number; name: string },
  browser: import("@playwright/test").Browser,
) {
  const ctx = await browser.newContext({
    viewport: { width: vp.w, height: vp.h },
    deviceScaleFactor: 2,
  });
  const page = await ctx.newPage();
  try {
    await page.goto(FRONTEND_URL, {
      waitUntil: "domcontentloaded",
      timeout: 15000,
    });
    await page.waitForTimeout(500);

    // Capture login screen first
    await page.screenshot({
      path: path.join(OUT_DIR, `${vp.name}-00-login.png`),
      fullPage: true,
    });

    const loggedIn = await login(page);
    if (!loggedIn) {
      console.log(`viewport ${vp.name}: login skipped (no token)`);
      return;
    }

    // Wait for layout to settle after login
    await page.waitForTimeout(800);
    await page.screenshot({
      path: path.join(OUT_DIR, `${vp.name}-01-after-login.png`),
      fullPage: true,
    });

    // Iterate top-level tabs by text label (resilient to data-* not existing)
    // Tab labels via i18n; matching by short fallback.
    const tabClicks = [
      { id: "claude", textCandidates: ["Claude", "claude"] },
      { id: "usage", textCandidates: ["Usage", "usage", "Cost", "Кост"] },
      { id: "config", textCandidates: ["Config", "Configuration", "config"] },
      { id: "token", textCandidates: ["Token", "Auth", "Logout", "token"] },
    ];

    for (const tab of tabClicks) {
      // Try clicking by various text candidates
      let clicked = false;
      for (const txt of tab.textCandidates) {
        if (await clickButtonByText(page, txt)) {
          clicked = true;
          break;
        }
      }
      if (!clicked) {
        // Try generic data attrs
        clicked = await clickTab(page, tab.id);
      }

      await page.waitForTimeout(600);
      await page.screenshot({
        path: path.join(OUT_DIR, `${vp.name}-${tab.id}.png`),
        fullPage: true,
      });

      // For usage tab, additionally capture sub-tabs
      if (tab.id === "usage") {
        for (const sub of USAGE_SUBTABS) {
          // Heuristic text candidates for sub-tabs
          let subText = sub;
          if (sub === "byCookie") subText = "Cookie";
          if (sub === "graveyard") subText = "Graveyard";
          if (sub === "overview") subText = "Overview";

          const sc = await clickButtonByText(page, subText);
          if (sc) {
            await page.waitForTimeout(600);
            await page.screenshot({
              path: path.join(OUT_DIR, `${vp.name}-usage-${sub}.png`),
              fullPage: true,
            });
          }
        }
        // Re-click usage so we end on usage tab proper before next top-tab cycle
      }
    }
  } catch (e) {
    console.error(`viewport ${vp.name} failed:`, e);
  } finally {
    await ctx.close();
  }
}

async function main() {
  await fs.mkdir(OUT_DIR, { recursive: true });
  const browser = await chromium.launch();
  for (const vp of VIEWPORTS) {
    console.log(`Capturing ${vp.name} (${vp.w}x${vp.h})...`);
    await captureForViewport(vp, browser);
  }
  await browser.close();
  console.log(`Done. Screenshots in ${OUT_DIR}`);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
