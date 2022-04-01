import { ChangeDirEvent, OpenInlineTrackEvent, RequestPlayTrackEvent } from "..";

import {
	BrowseApi,
	Configuration as SoilConfiguration,
	DirectoryEntryType
} from "../api/Soil";

export default class BrowseGrid extends HTMLElement {
	constructor() {
		super();
		const root = this.attachShadow({ mode: 'open' });

		root.innerHTML = `
			<style>
				ol {
					padding-left: 0;
				}

				li {
					list-style: none;
					border: 2px solid transparent;
				}

				a {
					all: unset;
					cursor: pointer;
					border: 2px solid transparent;
				}
				a:focus {
					background-color: var(--dark-grey);
					border: 2px solid var(--medium-grey);
				}

				@media (max-width: 768px) {
					li {
						font-size: 1.5rem;
					}
				}
			</style>
			<ol id="list"></ol>
		`;
	}

	static get observedAttributes() {
		return ['directory', 'inline-track'];
	}

	get directory(): string | null {
		return this.getAttribute('directory');
	}

	set directory(val: string | null) {
		if (val) {
			this.setAttribute('directory', val);
		} else {
			this.removeAttribute('directory');
		}
	}

	get inlineTrack(): string | null {
		return this.getAttribute('inline-track');
	}

	set inlineTrack(val: string | null) {
		if (val) {
			this.setAttribute('inline-track', val);
		} else {
			this.removeAttribute('inline-track');
		}
	}

	connectedCallback() {
		if (this.directory) {
			this.setDirectory(this.directory, 'dir');
		} else if (this.inlineTrack) {
			this.setDirectory(this.directory, 'inlineTracks');
		}
	}

	attributeChangedCallback(name: string, _oldValue: string | null, newValue: string | null) {
		switch (name) {
			case 'directory':
				this.setDirectory(newValue, 'dir');
				break;
			case 'inline-track':
				this.setDirectory(newValue, 'inlineTracks');
				break;
		}
	}

	private setDirectory(val: string | null, type: "dir" | "inlineTracks") {
		const r = this.shadowRoot;
		if (!r) {
			return;
		}
		const list = r.getElementById('list')!;
		if (!val) {
			list.innerHTML = '';
			return;
		} else {
			let promise;
			if (type === "dir") {
				promise = new BrowseApi(new SoilConfiguration({ basePath: '' }))
					.directoryIdGet({ id: val });

			} else {
				promise = new BrowseApi(new SoilConfiguration({ basePath: '' }))
					.inlineTracksIdGet({ id: val });
			}
			promise.then(ls => {
				if (!this.isConnected || !r) {
					return;
				}

				let olContent = "";
				let isFirst = true;
				if (ls.parent) {
					olContent += `
							<li>
								<a
									href="#"
									data-browse-id="${ls.parent.id}"
									data-type="dir"
									tabindex="1"
								>
									<browse-entry
										entry-type="dir"
										display-title=".."
									>
									</browse-entry>
								</a>
							</li>
						`;
					isFirst = false;

				}
				for (const l of ls.entries) {
					olContent += `
							<li>
								<a
									href="#"
									data-browse-id="${l.id}"
									data-type="${l.type}"
									tabindex="${isFirst ? '0' : '-1'}"
								>
									<browse-entry
										entry-type="${l.type}"
										display-title="${l.title}"
									>
									</browse-entry>
								</a>
							</li>
						`;
					isFirst = false;
				}
				list.innerHTML = olContent;

				const as = list.querySelectorAll('a');
				for (let i = 0; i < as.length; i++) {
					as[i].addEventListener('keydown', (evt) => {
						// TODO allow pressing first character(s) of item
						if (evt.code === 'Enter' || evt.code === 'Space') {
							evt.preventDefault();
							(<HTMLElement>evt.target)?.click();
						} else if (evt.code === 'ArrowRight' || evt.code === 'ArrowDown') {
							evt.preventDefault();
							let li = (evt.target as HTMLElement).parentElement as HTMLElement;
							let next = <HTMLElement | null | undefined>(li.nextElementSibling?.firstElementChild);
							if (!next) { // at end or only single item
								next = as[0];
							}
							(<HTMLElement>evt.target).tabIndex = -1;
							next.tabIndex = 0;
							next.focus();
						} else if (evt.code === 'ArrowLeft' || evt.code === 'ArrowUp') {
							evt.preventDefault();
							let li = (evt.target as HTMLElement).parentElement as HTMLElement;
							let prev = <HTMLElement | null | undefined>(li.previousElementSibling?.firstElementChild);
							if (!prev) { // at start or only single item
								prev = as[as.length - 1];
							}
							(<HTMLElement>evt.target).tabIndex = -1;
							prev.tabIndex = 0;
							prev.focus();
						} else if (evt.code === 'Home') {
							evt.preventDefault();
							const go = as[0];
							(<HTMLElement>evt.target).tabIndex = -1;
							go.tabIndex = 0;
							go.focus();
						} else if (evt.code === 'End') {
							evt.preventDefault();
							const go = as[as.length - 1];
							(<HTMLElement>evt.target).tabIndex = -1;
							go.tabIndex = 0;
							go.focus();
						}
					});
					as[i].addEventListener('click', (evt) => {
						evt.preventDefault();
						if (as[i].dataset.type === DirectoryEntryType.Dir) {
							const toEmit: ChangeDirEvent = {
								type: 'ChangeDir',
								to: as[i].dataset.browseId!
							};
							evt.target!.dispatchEvent(
								new CustomEvent(
									'navigateTo',
									{ bubbles: true, composed: true, detail: toEmit }
								)
							);
						} else if (as[i].dataset.type === DirectoryEntryType.InlineAudioFiles) {
							const toEmit: OpenInlineTrackEvent = {
								type: 'OpenInlineTrack',
								to: as[i].dataset.browseId!
							};
							evt.target!.dispatchEvent(
								new CustomEvent(
									'navigateTo',
									{ bubbles: true, composed: true, detail: toEmit }
								)
							);
						} else if (as[i].dataset.type === DirectoryEntryType.AudioFile ||
							as[i].dataset.type === DirectoryEntryType.InlineAudioFile) {
							const trackId = as[i].dataset.browseId!;
							const toEmit: RequestPlayTrackEvent = {
								type: 'RequestPlayTrack',
								trackId
							};
							evt.target!.dispatchEvent(
								new CustomEvent(
									'playerRequest',
									{ bubbles: true, composed: true, detail: toEmit }
								)
							);
						}
					});
				}
			});
		}
	}
}
