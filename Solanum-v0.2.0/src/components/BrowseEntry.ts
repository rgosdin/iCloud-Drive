import { DirectoryEntryType } from "../api/Soil";

export default class BrowseElement extends HTMLElement {
	constructor() {
		super();
		const root = this.attachShadow({ mode: 'open' });
		if (this.entryType === DirectoryEntryType.Dir ||
			this.entryType === DirectoryEntryType.InlineAudioFiles) {
			root.innerHTML = `
				<span id="symbol">🗀</span> <span id="title">${this.displayTitle}</span>
			`;
		} else if (this.entryType === DirectoryEntryType.AudioFile ||
			this.entryType === DirectoryEntryType.InlineAudioFile) {
			root.innerHTML = `
				<span id="symbol">▶</span> <span id="title">${this.displayTitle}</span>
			`;
		} else {
			root.innerHTML = `Unsupported Media type "${this.displayTitle}"`;
		}
	}

	static get observedAttributes() {
		return ['entry-type', 'display-title'];
	}

	get entryType(): string | null {
		return this.getAttribute('entry-type');
	}

	set entryType(val: string | null) {
		if (val) {
			this.setAttribute('entry-type', val);
		} else {
			this.removeAttribute('entry-type');
		}
	}

	get displayTitle(): string | null {
		return this.getAttribute('display-title');
	}

	set displayTitle(val: string | null) {
		if (val) {
			this.setAttribute('display-title', val);
		} else {
			this.removeAttribute('display-title');
		}
	}

	connectedCallback() {
	}

	attributeChangedCallback(name: string, _oldValue: string | null, newValue: string | null) {
		switch (name) {
			case 'entryType':
				break;
			case 'display-title':
				this.setDisplayTitle(newValue);
				break;
		}
	}

	private setDisplayTitle(val: string | null) {
		const r = this.shadowRoot;
		if (!r) {
			return;
		}
		const elem = r.getElementById('title');
		if (elem) {
			if (!val) {
				elem.innerText = '';
			} else {
				elem.innerText = val;
			}
		}
	}
}
