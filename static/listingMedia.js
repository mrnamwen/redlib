document.addEventListener("DOMContentLoaded", () => {
	const sizes = ["small", "medium", "large", "xlarge"];

	const setExpanded = (button, expanded) => {
		button.setAttribute("aria-expanded", expanded ? "true" : "false");
		button.textContent = expanded ? "Hide preview" : "Preview";
	};

	document.querySelectorAll(".listing_preview_toggle").forEach((button) => {
		button.addEventListener("click", () => {
			const panelId = button.getAttribute("aria-controls");
			if (!panelId) {
				return;
			}

			const panel = document.getElementById(panelId);
			if (!panel) {
				return;
			}

			const expanded = panel.hasAttribute("hidden");
			if (expanded) {
				panel.removeAttribute("hidden");
			} else {
				panel.setAttribute("hidden", "");
			}

			setExpanded(button, expanded);
		});
	});

	document.querySelectorAll(".listing_preview_resize").forEach((button) => {
		button.addEventListener("click", () => {
			const panel = button.closest(".listing_preview_panel");
			if (!panel) {
				return;
			}

			const step = Number(button.getAttribute("data-step") || "0");
			const current = panel.getAttribute("data-size") || "medium";
			const currentIndex = Math.max(0, sizes.indexOf(current));
			const nextIndex = Math.min(sizes.length - 1, Math.max(0, currentIndex + step));
			panel.setAttribute("data-size", sizes[nextIndex]);
		});
	});

	document.querySelectorAll(".listing_preview_reset").forEach((button) => {
		button.addEventListener("click", () => {
			const panel = button.closest(".listing_preview_panel");
			if (!panel) {
				return;
			}

			panel.setAttribute("data-size", "medium");
		});
	});
});
