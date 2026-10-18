import { invoke } from "@tauri-apps/api/core";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";

type CalendarEvent = {
  id: string;
  title: string;
  start: string;
  end: string;
  allDay: boolean;
  location?: string;
  color?: string;
};

type ViewMode = "day" | "week";

type AppStatus = { configured: boolean; connected: boolean; accountEmail?: string };
type EventCache = { events: CalendarEvent[]; syncedAt: string };

const state: { selectedDate: Date; view: ViewMode; events: CalendarEvent[] } = {
  selectedDate: startOfDay(new Date()),
  view: "day",
  events: createDemoEvents(),
};

function startOfDay(value: Date): Date {
  const date = new Date(value);
  date.setHours(0, 0, 0, 0);
  return date;
}

function addDays(value: Date, amount: number): Date {
  const date = new Date(value);
  date.setDate(date.getDate() + amount);
  return date;
}

function sameDay(a: Date, b: Date): boolean {
  return a.getFullYear() === b.getFullYear()
    && a.getMonth() === b.getMonth()
    && a.getDate() === b.getDate();
}

function at(dayOffset: number, hour: number, minute = 0): string {
  const date = addDays(startOfDay(new Date()), dayOffset);
  date.setHours(hour, minute, 0, 0);
  return date.toISOString();
}

function createDemoEvents(): CalendarEvent[] {
  return [
    { id: "welcome", title: "Chào mừng đến với LightCal", start: at(0, 9), end: at(0, 9, 30), allDay: false, color: "#6c5ce7" },
    { id: "focus", title: "Làm việc tập trung", start: at(0, 10), end: at(0, 11, 30), allDay: false, location: "Bàn làm việc", color: "#2f80ed" },
    { id: "review", title: "Xem lại kế hoạch tuần", start: at(1, 14), end: at(1, 14, 45), allDay: false, color: "#00a878" },
    { id: "deadline", title: "Hoàn thành bản nháp", start: at(3, 0), end: at(4, 0), allDay: true, color: "#e76f51" },
  ];
}

function formatDayHeading(date: Date): string {
  const text = new Intl.DateTimeFormat("vi-VN", { weekday: "long", day: "2-digit", month: "long", year: "numeric" }).format(date);
  return text.charAt(0).toUpperCase() + text.slice(1);
}

function formatTime(iso: string): string {
  return new Intl.DateTimeFormat("vi-VN", { hour: "2-digit", minute: "2-digit", hour12: false }).format(new Date(iso));
}

function escapeHtml(value: string): string {
  return value.replace(/[&<>'"]/g, (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", "'": "&#39;", '"': "&quot;" })[character] ?? character);
}

function eventsForDate(date: Date): CalendarEvent[] {
  return state.events.filter((event) => sameDay(new Date(event.start), date)).sort((a, b) => a.start.localeCompare(b.start));
}

function eventCard(event: CalendarEvent): string {
  const time = event.allDay ? "Cả ngày" : `${formatTime(event.start)} – ${formatTime(event.end)}`;
  const location = event.location ? `<span class="event-location">${escapeHtml(event.location)}</span>` : "";
  return `<article class="event-card" style="--event-color: ${event.color ?? "#6c5ce7"}">
    <span class="event-bar" aria-hidden="true"></span><div class="event-time">${time}</div>
    <div class="event-copy"><h3>${escapeHtml(event.title)}</h3>${location}</div></article>`;
}

function daySection(date: Date, compact = false): string {
  const events = eventsForDate(date);
  const weekday = new Intl.DateTimeFormat("vi-VN", { weekday: "short" }).format(date);
  const label = compact ? `<div class="date-tile${sameDay(date, new Date()) ? " is-today" : ""}"><span>${weekday}</span><strong>${date.getDate()}</strong></div>` : "";
  return `<section class="day-section${compact ? " compact" : ""}">${label}<div class="day-events">
    ${events.length ? events.map(eventCard).join("") : `<div class="empty-day">Không có lịch hẹn</div>`}
  </div></section>`;
}

function render(): void {
  const heading = document.querySelector<HTMLElement>("#date-heading");
  const content = document.querySelector<HTMLElement>("#calendar-content");
  if (!heading || !content) return;
  heading.textContent = state.view === "day" ? formatDayHeading(state.selectedDate) : `7 ngày từ ${new Intl.DateTimeFormat("vi-VN", { day: "2-digit", month: "2-digit" }).format(state.selectedDate)}`;
  content.innerHTML = state.view === "day" ? daySection(state.selectedDate) : Array.from({ length: 7 }, (_, index) => daySection(addDays(state.selectedDate, index), true)).join("");
  document.querySelectorAll<HTMLButtonElement>("[data-view]").forEach((button) => {
    const selected = button.dataset.view === state.view;
    button.classList.toggle("active", selected);
    button.setAttribute("aria-pressed", String(selected));
  });
}

function setSyncState(message: string, spinning = false): void {
  const label = document.querySelector<HTMLElement>("#sync-label");
  if (label) label.textContent = message;
  document.querySelector<HTMLElement>("#sync-icon")?.classList.toggle("spinning", spinning);
}

function setStatus(status: AppStatus): void {
  const title = document.querySelector<HTMLElement>("#account-title");
  const subtitle = document.querySelector<HTMLElement>("#account-subtitle");
  const connect = document.querySelector<HTMLButtonElement>("#connect-google");
  const disconnect = document.querySelector<HTMLButtonElement>("#disconnect-google");
  if (title) title.textContent = status.connected ? "Đã kết nối" : "Chưa kết nối";
  if (subtitle) subtitle.textContent = status.accountEmail ?? "Google Calendar";
  if (connect) connect.textContent = status.connected ? "Kết nối lại" : "Đăng nhập Google";
  disconnect?.classList.toggle("hidden", !status.connected);
}

function syncRange(): { timeMin: string; timeMax: string } {
  return { timeMin: addDays(startOfDay(new Date()), -7).toISOString(), timeMax: addDays(startOfDay(new Date()), 45).toISOString() };
}

async function syncFromGoogle(): Promise<void> {
  setSyncState("Đang đồng bộ Google Calendar…", true);
  try {
    const cache = await invoke<EventCache>("sync_calendar", syncRange());
    state.events = cache.events;
    render();
    setSyncState(`Đã đồng bộ lúc ${formatTime(cache.syncedAt)}`);
  } catch (error) {
    setSyncState(String(error));
  }
}

async function initializeGoogle(): Promise<void> {
  try {
    const cached = await invoke<EventCache | null>("load_cached_events");
    if (cached?.events.length) { state.events = cached.events; render(); setSyncState("Đang hiển thị dữ liệu đã lưu"); }
    const status = await invoke<AppStatus>("get_app_status");
    setStatus(status);
    if (status.connected) await syncFromGoogle();
  } catch {
    setSyncState("Chế độ xem thử · Mở bằng Tauri để kết nối Google");
  }
}

async function initializeAutostart(): Promise<void> {
  const checkbox = document.querySelector<HTMLInputElement>("#autostart");
  if (!checkbox) return;
  try { checkbox.checked = await isEnabled(); } catch { checkbox.disabled = true; }
}

window.addEventListener("DOMContentLoaded", () => {
  document.querySelector("#previous-day")?.addEventListener("click", () => { state.selectedDate = addDays(state.selectedDate, state.view === "week" ? -7 : -1); render(); });
  document.querySelector("#next-day")?.addEventListener("click", () => { state.selectedDate = addDays(state.selectedDate, state.view === "week" ? 7 : 1); render(); });
  document.querySelector("#today-button")?.addEventListener("click", () => { state.selectedDate = startOfDay(new Date()); render(); });
  document.querySelectorAll<HTMLButtonElement>("[data-view]").forEach((button) => button.addEventListener("click", () => { state.view = button.dataset.view as ViewMode; render(); }));
  document.querySelector("#sync-button")?.addEventListener("click", syncFromGoogle);
  document.querySelector("#settings-button")?.addEventListener("click", () => document.querySelector<HTMLDialogElement>("#settings-dialog")?.showModal());
  document.querySelector("#close-settings")?.addEventListener("click", () => document.querySelector<HTMLDialogElement>("#settings-dialog")?.close());
  document.querySelector<HTMLFormElement>("#client-id-form")?.addEventListener("submit", async (event) => {
    event.preventDefault();
    const input = document.querySelector<HTMLInputElement>("#client-id");
    const error = document.querySelector<HTMLElement>("#settings-error");
    if (!input || !error) return;
    error.textContent = "";
    try {
      const status = await invoke<AppStatus>("save_google_client_id", { clientId: input.value });
      setStatus(status);
      error.textContent = "Đã lưu Client ID trên máy.";
    } catch (reason) { error.textContent = String(reason); }
  });
  document.querySelector("#connect-google")?.addEventListener("click", async () => {
    const error = document.querySelector<HTMLElement>("#settings-error");
    if (error) error.textContent = "Trình duyệt đang mở để bạn đăng nhập…";
    try {
      const status = await invoke<AppStatus>("connect_google");
      setStatus(status);
      if (error) error.textContent = "Kết nối thành công.";
      await syncFromGoogle();
    } catch (reason) { if (error) error.textContent = String(reason); }
  });
  document.querySelector("#disconnect-google")?.addEventListener("click", async () => {
    try { setStatus(await invoke<AppStatus>("disconnect_google")); setSyncState("Đã ngắt kết nối Google"); } catch (error) { setSyncState(String(error)); }
  });
  document.querySelector<HTMLInputElement>("#autostart")?.addEventListener("change", async (event) => {
    const checkbox = event.currentTarget as HTMLInputElement;
    checkbox.disabled = true;
    try { if (checkbox.checked) await enable(); else await disable(); }
    catch (error) { checkbox.checked = !checkbox.checked; setSyncState(String(error)); }
    finally { checkbox.disabled = false; }
  });
  render();
  void initializeGoogle();
  void initializeAutostart();
});
