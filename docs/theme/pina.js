// Pina book chrome: brand marks in the menu bar and sidebar, the trimmed
// theme menu, and a page footer. Loaded through `additional-js`, after
// mdBook's own scripts, so every element it touches already exists.
(function pinaChrome() {
	"use strict";

	const root = typeof path_to_root === "string" ? path_to_root : "";
	const home = root === "" ? "./" : root;

	// The woven pineapple from `.github/assets/logo.svg`, drawn in
	// `currentColor` so CSS decides its colour.
	const logo =
		'<svg viewBox="0 0 320 320" aria-hidden="true" focusable="false"><g fill="currentColor" fill-rule="evenodd" transform="translate(-492.8 -69.9) scale(0.85 0.62)"><path d="M768 152 L793 205 Q795 210 792 217 L768 276 L744 217 Q741 210 744 204 Z"/><path d="M677 209 L721 223 Q730 226 734 235 L752 281 L708 267 Q698 264 694 254 Z"/><path d="M859 209 L815 223 Q806 226 802 235 L784 281 L828 267 Q838 264 842 254 Z"/><path d="M782 294 L724 294 Q706 292 693 306 L662 338 Q641 360 657 384 L676 404 Z"/><path d="M687 415 L793 304 Q816 281 837 307 L875 344 Q896 367 878 389 L862 405 L802 344 Q790 339 778 342 L875 439 Q897 463 878 494 L873 504 L768 397 L720 447 Z"/><path d="M669 418 L751 501 L716 537 L662 484 Q635 456 658 428 Z"/><path d="M760 581 L727 548 L810 464 L844 498 Z"/><path d="M661 505 L742 585 L719 585 Q690 584 670 563 Q645 545 655 521 Z"/><path d="M854 509 L869 524 Q880 546 864 565 Q850 585 822 586 Q803 588 787 579 Z"/></g></svg>';

	function brand(tag) {
		const link = document.createElement("a");
		link.className = "pina-brand";
		link.href = home;
		link.setAttribute("aria-label", "Pina home");
		link.innerHTML = logo + '<span class="pina-brand-name">pina</span>' +
			(tag ? `<span class="pina-brand-tag">${tag}</span>` : "");
		return link;
	}

	const title = document.querySelector(".menu-title");
	if (title) {
		title.textContent = "";
		title.appendChild(brand("docs"));
	}

	const scrollbox = document.querySelector(
		"#mdbook-sidebar .sidebar-scrollbox",
	);
	if (scrollbox && !scrollbox.querySelector(".pina-sidebar-head")) {
		const head = document.createElement("div");
		head.className = "pina-sidebar-head";
		head.appendChild(brand());
		const repo = document.createElement("a");
		repo.className = "pina-sidebar-link";
		repo.href = "https://github.com/pina-rs/pina";
		repo.textContent = "GitHub ↗";
		head.appendChild(repo);
		scrollbox.prepend(head);
	}

	const labels = {
		"mdbook-theme-default_theme": "Auto",
		"mdbook-theme-light": "Light",
		"mdbook-theme-navy": "Dark",
	};
	for (const [id, label] of Object.entries(labels)) {
		const button = document.getElementById(id);
		if (button) {
			button.textContent = label;
		}
	}

	const content = document.getElementById("mdbook-content");
	const main = content && content.querySelector("main");
	if (main && !content.querySelector(".pina-footer")) {
		const footer = document.createElement("footer");
		footer.className = "pina-footer";
		footer.innerHTML =
			'<span>Pina · Apache-2.0 · Built with mdBook</span><nav aria-label="Project links">' +
			'<a href="https://github.com/pina-rs/pina">GitHub</a>' +
			'<a href="https://crates.io/crates/pina">crates.io</a>' +
			'<a href="https://docs.rs/pina/">API reference</a>' +
			'<a href="https://github.com/pina-rs/pina/blob/main/SECURITY.md">Security</a>' +
			"</nav>";
		main.after(footer);
	}
})();
