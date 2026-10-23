// pina.rs landing page behaviour: theme switching shared with the book, tab
// strips, copy buttons, the navigation colour change, and scroll reveals.
// Everything here is progressive enhancement; the page reads fully without it.
(function landing() {
	"use strict";

	const root = document.documentElement;
	root.classList.remove("no-js");
	root.classList.add("js");

	// Theme. mdBook keeps its choice under `mdbook-theme`; the landing page
	// writes the same key so the book opens in the colours chosen here.
	const dark = window.matchMedia("(prefers-color-scheme: dark)");

	function effectiveTheme() {
		return root.dataset.theme || (dark.matches ? "dark" : "light");
	}

	const themeToggle = document.querySelector("[data-theme-toggle]");
	if (themeToggle) {
		themeToggle.addEventListener("click", () => {
			const next = effectiveTheme() === "dark" ? "light" : "dark";
			root.dataset.theme = next;
			try {
				localStorage.setItem(
					"mdbook-theme",
					next === "dark" ? "navy" : "light",
				);
			} catch (error) {
				// Storage can be unavailable in private windows; the toggle still
				// applies for this page view.
			}
		});
	}

	// Tab strips: the hero code window and the benchmark metrics.
	for (const tablist of document.querySelectorAll("[role='tablist']")) {
		const tabs = Array.from(tablist.querySelectorAll("[role='tab']"));

		function select(tab, focus) {
			for (const other of tabs) {
				const selected = other === tab;
				other.setAttribute("aria-selected", String(selected));
				other.tabIndex = selected ? 0 : -1;
				const panel = document.getElementById(
					other.getAttribute("aria-controls"),
				);
				if (panel) {
					panel.hidden = !selected;
					if (selected) {
						replayBars(panel);
					}
				}
			}
			if (focus) {
				tab.focus();
			}
		}

		tablist.addEventListener("click", (event) => {
			const tab = event.target.closest("[role='tab']");
			if (tab) {
				select(tab, false);
			}
		});

		tablist.addEventListener("keydown", (event) => {
			const index = tabs.indexOf(document.activeElement);
			if (index === -1) {
				return;
			}
			const moves = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 };
			if (event.key in moves) {
				event.preventDefault();
				select(
					tabs[(index + moves[event.key] + tabs.length) % tabs.length],
					true,
				);
			} else if (event.key === "Home") {
				event.preventDefault();
				select(tabs[0], true);
			} else if (event.key === "End") {
				event.preventDefault();
				select(tabs[tabs.length - 1], true);
			}
		});
	}

	// Benchmark bars grow in when their panel first comes into view, and again
	// whenever a different metric is chosen.
	function replayBars(panel) {
		const bars = panel.querySelector(".bars");
		if (!bars) {
			return;
		}
		bars.classList.remove("reveal-bars");
		void bars.offsetWidth;
		bars.classList.add("reveal-bars");
	}

	// Copy buttons.
	for (const button of document.querySelectorAll("[data-copy]")) {
		button.addEventListener("click", async () => {
			const source = button.closest("[data-copy-root]")?.querySelector(
				"[data-copy-source]",
			);
			if (!source) {
				return;
			}
			const label = button.querySelector(".copy-label");
			try {
				await navigator.clipboard.writeText(source.textContent.trim());
				button.classList.add("is-copied");
				if (label) {
					label.textContent = "Copied";
				}
			} catch (error) {
				if (label) {
					label.textContent = "Select";
				}
			}
			setTimeout(() => {
				button.classList.remove("is-copied");
				if (label) {
					label.textContent = "Copy";
				}
			}, 1600);
		});
	}

	if (!("IntersectionObserver" in window)) {
		return;
	}

	// The navigation sits on the dark hero, then takes the page colours once
	// the hero has scrolled away beneath it.
	const nav = document.querySelector("[data-nav]");
	const hero = document.querySelector(".hero");
	if (nav && hero) {
		new IntersectionObserver(([entry]) => {
			nav.classList.toggle("is-page", !entry.isIntersecting);
		}, { rootMargin: `-${nav.offsetHeight}px 0px 0px 0px` }).observe(hero);
	}

	// Scroll reveals. Anything already on screen is shown at once so nothing
	// above the fold flickers.
	const targets = document.querySelectorAll(
		".section-head, .card, .chart, .matrix-scroll, .commands li, .step, .next-link, .notice",
	);
	const reveal = new IntersectionObserver((entries) => {
		for (const entry of entries) {
			if (entry.isIntersecting) {
				entry.target.classList.add("is-in");
				reveal.unobserve(entry.target);
				if (entry.target.classList.contains("chart")) {
					const visible = entry.target.querySelector(
						".chart-panel:not([hidden])",
					);
					if (visible) {
						replayBars(visible);
					}
				}
			}
		}
	}, { rootMargin: "0px 0px -8% 0px" });

	for (const target of targets) {
		target.classList.add("reveal");
		if (target.getBoundingClientRect().top < window.innerHeight) {
			target.classList.add("is-in");
		} else {
			reveal.observe(target);
		}
	}
	root.classList.add("reveal-ready");
})();
