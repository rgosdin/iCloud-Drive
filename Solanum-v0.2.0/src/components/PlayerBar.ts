import { PlayPauseEvent, SeekTrackEvent, VolumeEvent } from "..";

export default class PlayerBar extends HTMLElement {
	constructor() {
		super();
		const root = this.attachShadow({ mode: 'open' });
		root.innerHTML = `
			<style>
				#bar {
					height: 6rem;
					background-color: var(--dark-grey);
				}

				#bar > div:nth-child(1) {
					background-image: linear-gradient(to right, var(--slider-lower-color), var(--slider-lower-color) 0%, var(--slider-upper-color) 0%);
					height: 7px;
				}

				#bar > div:nth-child(1):focus {
					outline: 1px dashed var(--light-grey);
				}

				#bar > div[tabindex="0"]:nth-child(1) {
					cursor: pointer;
				}

				#bar > div:nth-child(2) {
					display: flex;
					justify-content: space-between;
					box-sizing: border-box;
					height: 4rem;
				}

				#data {
					display: flex;
					justify-content: center;
				}

				#data > div:nth-child(1) {
					display: flex;
					flex-direction: column;
					justify-content: center;
				}

				#data > div:nth-child(2) {
					display: flex;
					align-items: center;
					margin-left: 2rem;
				}

				#volume {
					display: flex;
					align-items: center;
				}

				#volumeControl {
					height: 100%;
					width: 7px;
					background-image: linear-gradient(to top, var(--slider-lower-color), var(--slider-lower-color) 0%, var(--slider-upper-color) 0%);
					margin-left: 1rem;
					margin-right: 1rem;
					cursor: pointer;
				}

				#volumeControl:focus {
					outline: 1px dashed var(--light-grey);
				}

				button {
					width: 4rem;
					padding: 0;
					background: var(--light-grey);
					border: none;
					font-weight: bold;
					font-size: large;
					cursor: pointer;
				}

				button:disabled {
					cursor: unset;
				}

				button:focus {
					outline: 1px dashed var(--dark-grey);
				}

				@media (max-width: 768px) {
					#clock, #volumeLabel {
						display: none;
					}

					#volumeControl {
						width: 25px;
					}

					#data {
						font-size: .9rem;
					}
				}
			</style>
			<div id="bar">
				<div id="progress"></div>
				<div>
					<button id="playPause" tabindex="0"></button>
					<div id="data">
						<div>
							<div id="trackName"></div>
							<div style="visibility: hidden;">by <span id="artist"></span></div>
							<div style="visibility: hidden;">from <span id="album"></span></div>
						</div>
						<div id="clock" style="visibility: hidden;">
							<span id="clockCurrent"></span>/<span id="clockEnd"></span>
						</div>
					</div>
					<div id="volume">
						<span id="volumeLabel">Volume </span><div id="volumeControl" tabindex="0"></div>
					</div>
				</div>
			</div>
		`;
		root.getElementById('playPause')!.addEventListener('click', (evt) => {
			const toEmit: PlayPauseEvent = {
				type: 'PlayPause'
			};
			evt.target!.dispatchEvent(
				new CustomEvent(
					'playerRequest',
					{ bubbles: true, composed: true, detail: toEmit }
				)
			);
		});
		root.getElementById('volumeControl')!.addEventListener('keydown', (evt) => {
			let relativeOffset;
			if (evt.code === 'ArrowRight' || evt.code === 'ArrowUp') {
				relativeOffset = 1;
			} else if (evt.code === 'ArrowLeft' || evt.code === 'ArrowDown') {
				relativeOffset = -1;
			} else if (evt.code === 'PageUp') {
				relativeOffset = 5;
			} else if (evt.code === 'PageDown') {
				relativeOffset = -5;
			} else if (evt.code === 'Home') {
				relativeOffset = Number.NEGATIVE_INFINITY;
			} else if (evt.code === 'End') {
				relativeOffset = Number.POSITIVE_INFINITY;
			}

			if (relativeOffset) {
				const toEmit: VolumeEvent = {
					type: 'Volume',
					relativeOffset
				};
				evt.target!.dispatchEvent(
					new CustomEvent(
						'playerRequest',
						{ bubbles: true, composed: true, detail: toEmit }
					)
				);
			}
		});
		root.getElementById('volumeControl')!.addEventListener('click', (evt) => {
			const self = evt.target as HTMLDivElement;
			const { height } = self.getBoundingClientRect();

			const seekTo = (height - evt.offsetY) / height

			const toEmit: VolumeEvent = {
				type: 'Volume',
				absolute: seekTo * 100
			};
			evt.target!.dispatchEvent(
				new CustomEvent(
					'playerRequest',
					{ bubbles: true, composed: true, detail: toEmit }
				)
			);
		});
		root.getElementById('progress')!.addEventListener('keydown', (evt) => {
			let relativeOffset;
			if (evt.code === 'ArrowRight' || evt.code === 'ArrowUp') {
				relativeOffset = 1;
			} else if (evt.code === 'ArrowLeft' || evt.code === 'ArrowDown') {
				relativeOffset = -1;
			} else if (evt.code === 'PageUp') {
				relativeOffset = 5;
			} else if (evt.code === 'PageDown') {
				relativeOffset = -5;
			} else if (evt.code === 'Home') {
				relativeOffset = Number.NEGATIVE_INFINITY;
			}
			if (relativeOffset) {
				const toEmit: SeekTrackEvent = {
					type: 'SeekTrack',
					relativeOffset
				};
				evt.target!.dispatchEvent(
					new CustomEvent(
						'playerRequest',
						{ bubbles: true, composed: true, detail: toEmit }
					)
				);
			}
		})
		root.getElementById('progress')!.addEventListener('click', (evt) => {
			const self = evt.target as HTMLDivElement;
			if (!self.hasAttribute('tabindex')) {
				return;
			}

			const pos = evt.clientX;
			const { width } = self.getBoundingClientRect();
			const seekTo = parseInt(this.trackLength!) * (pos / width);

			const toEmit: SeekTrackEvent = {
				type: 'SeekTrack',
				absolute: seekTo
			};
			evt.target!.dispatchEvent(
				new CustomEvent(
					'playerRequest',
					{ bubbles: true, composed: true, detail: toEmit }
				)
			);
		});
	}
	static get observedAttributes() {
		return ['track-length', 'track-pos', 'play-state', 'gain', 'artist', 'track-name', 'album'];
	}

	get trackLength(): string | null {
		return this.getAttribute('track-length');
	}

	set trackLength(val: string | null) {
		if (val) {
			this.setAttribute('track-length', val);
		} else {
			this.removeAttribute('track-length');
		}
	}

	get trackPos(): string | null {
		return this.getAttribute('track-pos');
	}

	set trackPos(val: string | null) {
		if (val) {
			this.setAttribute('track-pos', val);
		} else {
			this.removeAttribute('track-pos');
		}
	}

	get playState(): string | null {
		return this.getAttribute('play-state');
	}

	set playState(val: string | null) {
		if (val) {
			this.setAttribute('play-state', val);
		} else {
			this.removeAttribute('play-state');
		}
	}

	get gain(): string | null {
		return this.getAttribute('gain');
	}

	set gain(val: string | null) {
		if (val) {
			this.setAttribute('gain', val);
		} else {
			this.removeAttribute('gain');
		}
	}

	get artist(): string | null {
		return this.getAttribute('artist');
	}

	set artist(val: string | null) {
		if (val) {
			this.setAttribute('artist', val);
		} else {
			this.removeAttribute('artist');
		}
	}

	get trackName(): string | null {
		return this.getAttribute('track-name');
	}

	set trackName(val: string | null) {
		if (val) {
			this.setAttribute('track-name', val);
		} else {
			this.removeAttribute('track-name');
		}
	}

	get album(): string | null {
		return this.getAttribute('album');
	}

	set album(val: string | null) {
		if (val) {
			this.setAttribute('album', val);
		} else {
			this.removeAttribute('album');
		}
	}

	connectedCallback() {
		this.updatePlayState();
	}

	attributeChangedCallback(name: string, _oldValue: string | null, _newValue: string | null) {
		switch (name) {
			case 'track-length':
			case 'track-pos':
				this.updateTrackPos();
				break;
			case 'play-state':
				this.updatePlayState();
				break;
			case 'gain':
				this.updateGain();
				break;
			case 'artist':
				this.updateArtist();
				break;
			case 'track-name':
				this.updateTrackName();
				break;
			case 'album':
				this.updateAlbum();
				break;
		}
	}

	private updateTrackName() {
		const r = this.shadowRoot;
		if (!this.isConnected || !r) {
			return;
		}

		const elem = r.getElementById('trackName') as HTMLSpanElement;
		elem.textContent = this.trackName;
	}

	private updateArtist() {
		const r = this.shadowRoot;
		if (!this.isConnected || !r) {
			return;
		}

		const name = this.artist;
		const elem = r.getElementById('artist') as HTMLDivElement;
		elem.textContent = name;
		if (!name) {
			(<HTMLDivElement>elem.parentNode).style.visibility = 'hidden';
		} else {
			(<HTMLDivElement>elem.parentNode).style.visibility = 'visible';
		}
	}

	private updateAlbum() {
		const r = this.shadowRoot;
		if (!this.isConnected || !r) {
			return;
		}

		const name = this.album;
		const elem = r.getElementById('album') as HTMLDivElement;
		elem.textContent = name;
		if (!name) {
			(<HTMLDivElement>elem.parentNode).style.visibility = 'hidden';
		} else {
			(<HTMLDivElement>elem.parentNode).style.visibility = 'visible';
		}
	}

	private updateGain() {
		const r = this.shadowRoot;
		if (!this.isConnected || !r) {
			return;
		}

		const volume = r.getElementById('volumeControl') as HTMLDivElement;
		const percent = this.gain ? this.gain : 0;
		volume.style.background =
			`linear-gradient(to top, var(--slider-lower-color), var(--slider-lower-color) ${percent}%, var(--slider-upper-color) ${percent}%)`;
	}

	private updateTrackPos() {
		const r = this.shadowRoot;
		if (!this.isConnected || !r) {
			return;
		}

		function formatTime(seconds: number) {
			const hours = Math.floor(seconds / 3600);
			const minutes = Math.floor(seconds / 60) % 60;
			const secs = Math.floor(seconds % 60);

			return [hours, minutes, secs]
				.map(v => v < 10 ? "0" + v : v)
				.filter((v, i) => v !== "00" || i > 0)
				.join(":")
		}

		const progress = r.getElementById('progress') as HTMLDivElement;
		const clockCurrent = r.getElementById('clockCurrent') as HTMLSpanElement;
		const clockEnd = r.getElementById('clockEnd') as HTMLSpanElement;
		const clock = clockCurrent.parentNode as HTMLDivElement;
		const pos = this.trackPos;
		const length = this.trackLength;

		let percent = 0;
		if (pos && length) {
			const posInt = parseInt(pos);
			const lengthInt = parseInt(length);
			percent = 100 * (posInt / lengthInt);
			clockCurrent.textContent = formatTime(posInt);
			clockEnd.textContent = formatTime(lengthInt);
			clock.style.visibility = 'visible';
		} else {
			clockCurrent.textContent = '';
			clockEnd.textContent = '';
			clock.style.visibility = 'hidden';
		}
		progress.style.background =
			`linear-gradient(to right, var(--slider-lower-color), var(--slider-lower-color) ${percent}%, var(--slider-upper-color) ${percent}%)`;
	}

	private updatePlayState() {
		const r = this.shadowRoot;
		if (!this.isConnected || !r) {
			return;
		}
		let btnText;
		const btn = r.getElementById('playPause') as HTMLButtonElement;
		const progress = r.getElementById('progress') as HTMLDivElement;
		if (this.playState === 'playing') {
			btnText = '||';
			progress.tabIndex = 0;
			btn.disabled = false;
		} else if (this.playState === 'paused') {
			btnText = '|>'
			progress.tabIndex = 0;
			btn.disabled = false;
		} else {
			btnText = '[]';
			progress.removeAttribute('tabIndex');
			btn.disabled = true;
		}
		btn.textContent = `${btnText}`;
	}
}
