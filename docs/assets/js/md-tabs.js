// md-components v1 (draft). Optional progressive enhancement: inside
// section[data-tabs], consecutive h3 + code block pairs become ARIA tabs.
// Without JS every method stays visible in order.
document.querySelectorAll("section[data-tabs]").forEach((sec) => {
  const heads = [...sec.querySelectorAll(":scope > h3")].filter((h) =>
    h.nextElementSibling?.matches(".highlighter-rouge, pre"));
  if (heads.length < 2) return;
  const list = document.createElement("div");
  list.setAttribute("role", "tablist");
  list.setAttribute("aria-label", sec.dataset.tabsLabel || "Options");
  heads[0].before(list);
  heads.forEach((h, i) => {
    const panel = h.nextElementSibling, id = h.id || `md-tab-${i}`;
    const b = document.createElement("button");
    b.type = "button"; b.role = "tab"; b.id = `${id}-tab`; b.textContent = h.textContent;
    panel.id ||= `${id}-panel`; panel.setAttribute("role", "tabpanel");
    panel.setAttribute("aria-labelledby", b.id); b.setAttribute("aria-controls", panel.id);
    const sel = (on) => { b.setAttribute("aria-selected", on); b.tabIndex = on ? 0 : -1; panel.hidden = !on; };
    b.addEventListener("click", () => list.querySelectorAll("[role=tab]").forEach((t) => t._sel(t === b)));
    b._sel = sel; sel(i === 0); h.hidden = true; list.append(b);
  });
  list.addEventListener("keydown", (e) => {
    const tabs = [...list.children], i = tabs.indexOf(document.activeElement);
    const d = e.key === "ArrowRight" ? 1 : e.key === "ArrowLeft" ? -1 : 0;
    if (!d || i < 0) return;
    const n = tabs[(i + d + tabs.length) % tabs.length]; tabs.forEach((t) => t._sel(t === n)); n.focus();
  });
});
