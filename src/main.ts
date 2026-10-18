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

window.addEventListener("DOMContentLoaded", () => {
  document.querySelector("#previous-day")?.addEventListener("click", () => { state.selectedDate = addDays(state.selectedDate, state.view === "week" ? -7 : -1); render(); });
  document.querySelector("#next-day")?.addEventListener("click", () => { state.selectedDate = addDays(state.selectedDate, state.view === "week" ? 7 : 1); render(); });
  document.querySelector("#today-button")?.addEventListener("click", () => { state.selectedDate = startOfDay(new Date()); render(); });
  document.querySelectorAll<HTMLButtonElement>("[data-view]").forEach((button) => button.addEventListener("click", () => { state.view = button.dataset.view as ViewMode; render(); }));
  document.querySelector("#sync-button")?.addEventListener("click", () => { setSyncState("Đang đồng bộ…", true); window.setTimeout(() => setSyncState("Dữ liệu mẫu · Kết nối Google trong Cài đặt"), 650); });
  document.querySelector("#settings-button")?.addEventListener("click", () => document.querySelector<HTMLDialogElement>("#settings-dialog")?.showModal());
  document.querySelector("#close-settings")?.addEventListener("click", () => document.querySelector<HTMLDialogElement>("#settings-dialog")?.close());
  render();
});
