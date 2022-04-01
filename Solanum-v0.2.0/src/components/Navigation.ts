import { ChangeLibraryEvent } from "..";

import {
	BrowseApi,
	Configuration as SoilConfiguration,
} from "../api/Soil";

export default class Navigation extends HTMLElement {
	constructor() {
		super();
		const root = this.attachShadow({ mode: 'open' });
		root.innerHTML = `
			<style>
				nav h1 {
					margin: 0.5rem;
				}
				a {
					all: unset;
				}
				nav ol {
					all: unset;
					list-style: none;
					display: flex;
					justify-content: center;
				}
				nav li {
					margin-right: 2rem;
					border: 2px solid transparent;
					cursor: pointer;
				}
				nav li:last-child {
					margin-right: 0;
				}
				nav li.current {
					background-color: var(--dark-grey);
				}
				nav li.focus {
					background-color: var(--dark-grey);
					border: 2px solid var(--medium-grey);
				}
			</style>
			<nav>
				<ol id="list">
				</ol>
			</nav>
		`;
	}

	static get observedAttributes() {
		return ['current'];
	}

	get current(): string | null {
		return this.getAttribute('current');
	}

	set current(val: string | null) {
		if (val) {
			this.setAttribute('current', val);
		} else {
			this.removeAttribute('current');
		}
	}

	connectedCallback() {
		new BrowseApi(new SoilConfiguration({ basePath: '' })).librariesGet().then(ls => {
			const r = this.shadowRoot;
			if (!this.isConnected || !r) {
				return;
			}
			const ol = r.getElementById('list') as HTMLOListElement;
			let olContent = "";
			let isFirst = true;
			for (const l of ls) {
				olContent += `
					<li>
						<h1>
							<a
								tabindex="${isFirst ? '0' : '-1'}"
								data-library="${l.id}"
							>
								${l.title}
							</a>
						</h1>
					</li>
				`;
				isFirst = false;
			}
			ol.innerHTML = olContent;

			const as = ol.querySelectorAll('a');
			for (let i = 0; i < as.length; i++) {
				as[i].addEventListener('focus', evt => {
					const li = (evt.target as HTMLElement).parentElement!.parentElement as HTMLElement;
					li.classList.add('focus');
				});
				as[i].addEventListener('blur', evt => {
					const li = (evt.target as HTMLElement).parentElement!.parentElement as HTMLElement;
					li.classList.remove('focus');
				});
				as[i].addEventListener('keydown', (evt) => {
					//TODO allow pressing the first character of menu item
					if (evt.code === 'Enter' || evt.code === 'Space') {
						evt.preventDefault();
						(<HTMLElement>evt.target)?.click();
					} else if (evt.code === 'ArrowRight' || evt.code === 'ArrowDown') {
						const li = (evt.target as HTMLElement)
							.parentElement!.parentElement as HTMLElement;
						let next =
							<HTMLElement | null | undefined>
							(li.nextElementSibling?.firstElementChild?.firstElementChild);
						if (!next) { // at end
							next = as[0];
						}
						(<HTMLElement>evt.target).tabIndex = -1;
						next.tabIndex = 0;
						next.focus();
					} else if (evt.code === 'ArrowLeft' || evt.code === 'ArrowUp') {
						const li = (evt.target as HTMLElement)
							.parentElement!.parentElement as HTMLElement;
						let prev =
							<HTMLElement | null | undefined>
							(li.previousElementSibling?.firstElementChild?.firstElementChild);
						if (!prev) { // at start
							prev = as[as.length - 1];
						}
						(<HTMLElement>evt.target).tabIndex = -1;
						prev.tabIndex = 0;
						prev.focus();
					} else if (evt.code === 'Home') {
						const go = as[0];
						(<HTMLElement>evt.target).tabIndex = -1;
						go.tabIndex = 0;
						go.focus();
					} else if (evt.code === 'End') {
						const go = as[as.length - 1];
						(<HTMLElement>evt.target).tabIndex = -1;
						go.tabIndex = 0;
						go.focus();
					}
				});
				as[i].addEventListener('click', (evt) => {
					evt.preventDefault();
					const we = evt.target as HTMLLinkElement;
					const toEmit: ChangeLibraryEvent = {
						type: 'ChangeLibrary',
						library: we.dataset.library!
					};
					we.dispatchEvent(
						new CustomEvent(
							'navigateTo',
							{ bubbles: true, composed: true, detail: toEmit }));
				});
			}
			const current = this.getAttribute('current');
			if (!current) {
				const firstElement = ls.values().next()
				if (firstElement) {
					this.current = firstElement.value.id;
				}
			} else {
				this.setLibrary(this.getAttribute('current'));
			}
		});
	}

	attributeChangedCallback(name: string, _oldValue: string | null, newValue: string | null) {
		switch (name) {
			case 'current':
				this.setLibrary(newValue);
				break;
		}
	}

	private setLibrary(val: string | null) {
		const r = this.shadowRoot;
		if (!r) {
			return;
		}
		const lis = r.querySelectorAll('li');
		for (let i = 0; i < lis.length; i++) {
			const li = lis[i];
			const a = li.firstElementChild!.firstElementChild as HTMLElement;
			const classes = li.classList;
			if (a.dataset.library === val) {
				classes.add('current');
			} else {
				classes.remove('current');
			}
		}
	}
}
