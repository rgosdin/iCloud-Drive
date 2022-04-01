import Navigation from "./components/Navigation";
import BrowseGrid from "./components/BrowseGrid";
import BrowseEntry from "./components/BrowseEntry";
import PlayerBar from "./components/PlayerBar";
import { TrackMetaData } from "./api/Soil";

export interface HistoryState {
	contentScrollTop: number;
}

export interface SolanumEvent {
	type: string;
}

export interface ChangeLibraryEvent extends SolanumEvent {
	type: 'ChangeLibrary';
	library: string;
}

export interface ChangeDirEvent extends SolanumEvent {
	type: 'ChangeDir',
	to: string;
}

export interface OpenInlineTrackEvent extends SolanumEvent {
	type: 'OpenInlineTrack',
	to: string;
}

export interface RequestPlayTrackEvent extends SolanumEvent {
	type: 'RequestPlayTrack',
	trackId: string;
}

export interface SeekTrackEvent extends SolanumEvent {
	type: 'SeekTrack',
	relativeOffset?: number
	absolute?: number
}

export interface PlayPauseEvent extends SolanumEvent {
	type: 'PlayPause'
}

export interface VolumeEvent extends SolanumEvent {
	type: 'Volume',
	relativeOffset?: number,
	absolute?: number
}

class Player {
	private currentAudioElement?: HTMLAudioElement;
	private currentDuration?: number;
	private gain: number = 0.5;

	constructor() {
		(<PlayerBar>document.getElementById('player')).gain = String(this.gain * 100);
	}

	playPause() {
		if (!this.currentAudioElement) {
			return;
		}

		const playerBar = (<PlayerBar>document.getElementById('player'));
		if (this.currentAudioElement.paused) {
			this.currentAudioElement.play();
			playerBar.playState = 'playing';
		} else {
			this.currentAudioElement.pause();
			playerBar.playState = 'paused';
		}
	}

	volumeRelative(relativeOffset: number) {
		const adjustedOffset = relativeOffset / 100;
		const newGain = Math.min(Math.max(this.gain + adjustedOffset, 0), 1)
		this.gain = newGain;
		(<PlayerBar>document.getElementById('player')).gain = String(this.gain * 100);

		if (!this.currentAudioElement || !this.currentDuration) {
			return;
		}
		this.currentAudioElement.volume = this.gain;
	}

	volume(absolute: number) {
		const adjustedAbsolute = absolute / 100;
		this.gain = adjustedAbsolute;
		(<PlayerBar>document.getElementById('player')).gain = String(this.gain * 100);

		if (!this.currentAudioElement || !this.currentDuration) {
			return;
		}
		this.currentAudioElement.volume = this.gain;
	}

	seekRelative(relativeOffset: number) {
		if (!this.currentAudioElement || !this.currentDuration) {
			return;
		}

		// TODO disable everything while seeking
		let newTime = this.currentAudioElement.currentTime;
		newTime += relativeOffset;
		newTime = Math.min(newTime, this.currentDuration);
		newTime = Math.max(newTime, 0);
		this.currentAudioElement.currentTime = newTime;
	}

	seek(absolute: number) {
		if (!this.currentAudioElement || !this.currentDuration) {
			return;
		}

		this.currentAudioElement.currentTime = Math.min(Math.max(absolute, 0), this.currentDuration);
	}

	play(trackId: string) {
		const playerBar = (<PlayerBar>document.getElementById('player'));

		function updatePlayer(
			meta: TrackMetaData | null,
			pos: string | null,
			state: string | null) {
			if (meta) {
				playerBar.artist = meta.artistName ? meta.artistName : null;
				playerBar.trackName = meta.title;
				playerBar.album = meta.albumTitle;
				playerBar.trackLength = String(meta.duration);
			} else {
				playerBar.artist = null;
				playerBar.trackName = null;
				playerBar.album = null;
				playerBar.trackLength = null;
			}
			playerBar.trackPos = pos;
			playerBar.playState = state;
		}

		if (this.currentAudioElement) {
			this.currentAudioElement.src = '';
			this.currentAudioElement?.load();
			this.currentAudioElement = undefined;
			this.currentDuration = undefined;
			updatePlayer(null, null, null);
		}

		fetch(`/track/${encodeURIComponent(trackId)}`, {
			headers: {
				Accept: 'application/json'
			}
		}).then((resp) => {
			resp.json().then((json: TrackMetaData) => {
				this.currentDuration = json.duration;
				updatePlayer(json, '0', null);
			});
		});

		const ctx = new AudioContext();
		this.currentAudioElement = new Audio(`/track/${encodeURIComponent(trackId)}`);
		this.currentAudioElement.volume = this.gain;
		this.currentAudioElement?.addEventListener('canplaythrough', () => {
			const srcNode = ctx.createMediaElementSource(this.currentAudioElement!);
			srcNode.connect(ctx.destination);

			if (ctx.state === 'suspended') {
				ctx.resume();
			}

			this.currentAudioElement?.play();
			playerBar.playState = 'playing';
			this.doRenderUpdate();
		});
		this.currentAudioElement?.addEventListener('ended', () => {
			this.currentAudioElement = undefined;
			this.currentDuration = undefined;
			updatePlayer(null, null, null);
		});
	}

	private doRenderUpdate() {
		if (this.currentAudioElement) {
			const playerBar = (<PlayerBar>document.getElementById('player'));
			playerBar.trackPos = String(this.currentAudioElement.currentTime);
			window.requestAnimationFrame(this.doRenderUpdate.bind(this));
		}
	}
}

export default class Solanum {
	private readonly root = document.body;
	private readonly player = new Player();

	async init() {
		window.history.scrollRestoration = 'manual';

		window.addEventListener('hashchange', this.onNavChange.bind(this));
		window.addEventListener('popstate', (evt) => {
			this.onNavChange();
			const state = evt.state;
			if (state) {
				const offset = (<HistoryState>state).contentScrollTop;
				document.getElementById('content')!.scroll(0, offset);
			}
		});

		//window.setInterval(this.saveScrollPosition.bind(this), 50);
		this.root.addEventListener('playerRequest', (evt) => {
			const sevt = (<SolanumEvent>(<CustomEvent>evt).detail);
			switch (sevt.type) {
				case 'RequestPlayTrack': {
					const req = sevt as RequestPlayTrackEvent;
					this.player.play(req.trackId);
					break;
				}
				case 'SeekTrack': {
					const req = sevt as SeekTrackEvent;
					if (req.relativeOffset) {
						this.player.seekRelative(req.relativeOffset);
					} else if (req.absolute) {
						this.player.seek(req.absolute);
					}
					break;
				}
				case 'PlayPause':
					this.player.playPause();
					break;
				case 'Volume': {
					const req = sevt as VolumeEvent;
					if (req.relativeOffset) {
						this.player.volumeRelative(req.relativeOffset);
					} else if (req.absolute) {
						this.player.volume(req.absolute);
					}
					break;
				}
			}
		});
		this.root.addEventListener('navigateTo', (evt) => {
			const state: HistoryState = {
				contentScrollTop: document.getElementById('content')!.scrollTop
			}
			const sevt = (<SolanumEvent>(<CustomEvent>evt).detail);
			switch (sevt.type) {
				case 'ChangeLibrary':
					{
						const to = <ChangeLibraryEvent>sevt;
						const searchParams = new URLSearchParams(window.location.search);
						searchParams.set('library', to.library);
						searchParams.delete('dir');
						searchParams.delete('inlineTrack');

						const newUrl = window.location.protocol + "//" + window.location.host + window.location.pathname + '?' + searchParams.toString();
						window.history.pushState(state, '', newUrl);
						this.onNavChange();
					}
					break;
				case 'ChangeDir':
					{
						const to = <ChangeDirEvent>sevt;
						const searchParams = new URLSearchParams(window.location.search);
						searchParams.set('dir', to.to);
						searchParams.delete('inlineTrack');

						const newUrl = window.location.protocol + "//" + window.location.host + window.location.pathname + '?' + searchParams.toString();
						window.history.pushState(state, '', newUrl);
						this.onNavChange();
					}
					break;
				case 'OpenInlineTrack':
					{
						const to = <OpenInlineTrackEvent>sevt;
						const searchParams = new URLSearchParams(window.location.search);
						searchParams.set('inlineTrack', to.to);
						searchParams.delete('dir');

						const newUrl = window.location.protocol + "//" + window.location.host + window.location.pathname + '?' + searchParams.toString();
						window.history.pushState(state, '', newUrl);
						this.onNavChange();
					}
					break;
			}
		})

		this.onNavChange();
	}

	private onNavChange() {
		const nav = document.getElementById('nav') as Navigation;
		const content = document.getElementById('content') as BrowseGrid;
		const query = new URLSearchParams(window.location.search);
		const library = query.get('library');
		if (library) {
			nav.current = library;
		}

		const directory = query.get('dir');
		if (directory) {
			content.directory = directory;
		} else if (query.get('inlineTrack')) {
			content.directory = null;
			content.inlineTrack = query.get('inlineTrack');
		} else {
			if (nav.current) {
				content.directory = nav.current;
			} else {
				new MutationObserver(function(mutationList, observer) {
					for (const m of mutationList) {
						if (m.type === 'attributes' && m.attributeName === 'current') {
							content.directory = nav.current;
							observer.disconnect();
						}
					}
				}).observe(nav, {
					attributeFilter: ['current']
				});
			}
		}
	}

	//	private saveScrollPosition(): void {
	//		const topOffset = document.getElementById('content')!.scrollTop;
	//		window.history.replaceState(
	//			{ contentScrollTop: topOffset } as HistoryState, '',
	//			window.location.href);
	//	}
}

window.customElements.define('solanum-navigation', Navigation);
window.customElements.define('browse-grid', BrowseGrid);
window.customElements.define('browse-entry', BrowseEntry);
window.customElements.define('player-bar', PlayerBar);
document.addEventListener('DOMContentLoaded', () => new Solanum().init());
