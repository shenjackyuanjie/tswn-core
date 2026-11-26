/** 四列 Boss 下拉表；隐藏 select 保留原有挑战参数与单选语义。 */
export function setupBossPicker(select, toggle, loadIcon) {
  const trigger = document.createElement("button");
  trigger.type = "button";
  trigger.className = "boss-picker-trigger";
  trigger.setAttribute("aria-haspopup", "listbox");
  trigger.setAttribute("aria-expanded", "false");
  const previewIcon = document.createElement("img");
  previewIcon.alt = "";
  previewIcon.width = previewIcon.height = 24;
  const previewName = document.createElement("span");
  const arrow = document.createElement("span");
  arrow.textContent = "▾";
  arrow.className = "boss-picker-arrow";
  arrow.setAttribute("aria-hidden", "true");
  trigger.append(previewIcon, previewName, arrow);
  const list = document.createElement("div");
  list.className = "boss-choice-list";
  list.id = "bossChoiceList";
  list.hidden = true;
  list.setAttribute("role", "listbox");
  list.setAttribute("aria-label", "挑战 Boss");
  trigger.setAttribute("aria-controls", list.id);
  select.hidden = true;
  select.insertAdjacentElement("afterend", trigger);
  // 弹层放到 body，避免右侧滚动栏裁掉向左展开的四列表格。
  document.body.append(list);
  const buttons = [...select.options].map(option => {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "boss-choice";
    button.setAttribute("role", "option");
    button.dataset.boss = option.value;
    button.title = option.value + "@!";
    const icon = document.createElement("img");
    icon.alt = "";
    icon.width = icon.height = 24;
    const label = document.createElement("span");
    label.textContent = option.textContent;
    button.append(icon, label);
    button.addEventListener("click", () => {
      select.value = option.value;
      select.dispatchEvent(new Event("change", {bubbles:true}));
    });
    list.append(button);
    // 图标读取失败不阻断选择或战斗。
    Promise.resolve().then(() => loadIcon(option.value)).then(url => {
      if (url) icon.src = url;
      if (select.value === option.value) sync();
    }).catch(() => { icon.hidden = true; });
    return button;
  });
  function close() {
    list.hidden = true;
    trigger.setAttribute("aria-expanded", "false");
  }
  function position() {
    const rect = trigger.getBoundingClientRect();
    const width = Math.min(520, Math.max(0, window.innerWidth - 24));
    list.style.width = width + "px";
    list.style.left = Math.max(12, Math.min(rect.right-width, window.innerWidth-width-12)) + "px";
    list.style.top = Math.max(12, Math.min(rect.bottom+6, window.innerHeight-180)) + "px";
    list.style.maxHeight = Math.max(100, window.innerHeight-parseFloat(list.style.top)-12) + "px";
  }
  function open() {
    if (!toggle.checked) return;
    position();
    list.hidden = false;
    trigger.setAttribute("aria-expanded", "true");
  }
  function sync() {
    trigger.disabled = !toggle.checked;
    const selected = buttons.find(button => button.dataset.boss === select.value);
    previewName.textContent = select.selectedOptions[0]?.textContent ?? "选择 Boss";
    trigger.title = select.value + "@!";
    if (selected?.querySelector("img")?.getAttribute("src")) {
      previewIcon.src = selected.querySelector("img").getAttribute("src");
      previewIcon.hidden = false;
    } else previewIcon.hidden = true;
    for (const button of buttons) {
      button.disabled = !toggle.checked;
      button.setAttribute("aria-selected", String(button.dataset.boss === select.value));
    }
    if (!toggle.checked) close();
  }
  trigger.addEventListener("click", () => list.hidden ? open() : close());
  trigger.addEventListener("keydown", event => {
    if (event.key !== "ArrowDown") return;
    event.preventDefault();
    open();
    buttons.find(button => button.dataset.boss === select.value)?.focus();
  });
  list.addEventListener("keydown", event => {
    if (event.key === "Escape") {
      close(); trigger.focus(); return;
    }
    const current = buttons.indexOf(document.activeElement);
    if (current < 0 || !["ArrowUp","ArrowDown","ArrowLeft","ArrowRight","Home","End"].includes(event.key)) return;
    event.preventDefault();
    const offsets = {ArrowUp:-4,ArrowDown:4,ArrowLeft:-1,ArrowRight:1};
    const next = event.key === "Home" ? 0 : event.key === "End" ? buttons.length-1 :
      Math.max(0, Math.min(buttons.length-1, current+offsets[event.key]));
    buttons[next]?.focus();
  });
  document.addEventListener("pointerdown", event => {
    if (!trigger.contains(event.target) && !list.contains(event.target)) close();
  });
  window.addEventListener("resize", close);
  document.addEventListener("scroll", event => {
    // 日志自动滚动不应打断选择；只有选择器自身祖先滚动时才关闭。
    if (event.target !== list && event.target.contains?.(trigger)) close();
  }, true);
  toggle.addEventListener("change", sync);
  select.addEventListener("change", sync);
  sync();
  return {element:list, trigger, sync, close};
}
