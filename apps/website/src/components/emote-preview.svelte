<script lang="ts">
	import type { Emote, EmoteSetEmote } from "$/gql/graphql";
	import Flags, {
		emoteToFlags,
		determineHighlightColor,
		emoteSetEmoteToFlags,
	} from "./flags.svelte";
	import Checkbox from "./input/checkbox.svelte";
	import ResponsiveImage from "./responsive-image.svelte";
	import type { HTMLAttributes } from "svelte/elements";
	import UserName from "./user-name.svelte";
	import EmoteContextMenu from "./emote-context-menu.svelte";
	import { defaultEmoteSet } from "$/lib/defaultEmoteSet";
	import { editableEmoteSets } from "$/lib/emoteSets";
	import Spinner from "./spinner.svelte";

	type Props = {
		data: Emote;
		emoteSetEmote?: EmoteSetEmote;
		index?: number;
		bg?: "medium" | "light";
		emoteOnly?: boolean;
		selectionMode?: boolean;
		selected?: boolean;
		ignoredFlagsForHighlight?: string[];
	} & HTMLAttributes<HTMLAnchorElement>;

	let {
		data,
		emoteSetEmote,
		index = 0,
		bg = "medium",
		emoteOnly = false,
		selectionMode = false,
		selected = $bindable(false),
		ignoredFlagsForHighlight = [],
		...restProps
	}: Props = $props();

	let flags = $derived(
		emoteSetEmote
			? emoteSetEmoteToFlags(emoteSetEmote, $defaultEmoteSet, $editableEmoteSets)
			: emoteToFlags(data, $defaultEmoteSet, $editableEmoteSets),
	);

	let highlight = $derived(determineHighlightColor(flags, ignoredFlagsForHighlight));

	let title = $derived.by(() => {
		let title = data.defaultName;
		const owner = data.owner?.mainConnection?.platformDisplayName;

		if (owner) {
			title += ` by ${owner}`;
		}

		return title;
	});

	function onClick(e: MouseEvent) {
		if (selectionMode) {
			selected = !selected;
			e.preventDefault();
		}
	}

	let menuPosition: { x: number; y: number } | undefined = $state();

	function onContextMenu(e: MouseEvent) {
		e.preventDefault();
		menuPosition = { x: e.clientX, y: e.clientY };
	}
</script>

<EmoteContextMenu {data} bind:position={menuPosition} />
<a
	href="/emotes/{data.id}"
	data-sveltekit-preload-data="tap"
	class="emote"
	class:emote-only={emoteOnly}
	class:selected={selectionMode && selected}
	draggable={!selected}
	style={highlight
		? `--highlight: ${highlight}80; --highlight-active: ${highlight};`
		: "--highlight: transparent; --highlight-active: var(--border-active);"}
	style:background-color="var(--bg-{bg})"
	onclick={onClick}
	oncontextmenu={onContextMenu}
	{title}
	{...restProps}
>
	{#if data.imagesPending}
		<Spinner />
	{:else}
		<ResponsiveImage images={data.images} {index} />
	{/if}
	{#if !emoteOnly}
		<span class="name">{emoteSetEmote?.alias ?? data.defaultName}</span>
		{#if data.owner?.mainConnection?.platformDisplayName}
			<span class="user">
				<UserName user={data.owner} />
			</span>
		{/if}
	{/if}
	{#if selectionMode || flags.length > 0}
		<div class="flags">
			{#if selectionMode}
				<Checkbox bind:value={selected} />
			{/if}
			{#if !emoteOnly && flags.length > 0}
				<Flags {flags} iconOnly style="flex-direction: column; gap: 0.4rem;" />
			{/if}
		</div>
	{/if}
</a>

<style lang="scss">
	.emote {
		position: relative;

		color: var(--text);
		text-decoration: none;

		width: 100%;
		max-width: 10rem;
		aspect-ratio: 1 / 1;

		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;

		border: 1px solid transparent;
		border-radius: 0.5rem;
		cursor: pointer;
		user-select: none;

		border-color: var(--highlight);

		&:hover,
		&:focus-visible {
			border-color: var(--highlight-active);
		}

		&.selected {
			border-color: var(--primary);
		}

		& > :global(picture) {
			flex-grow: 1;
			margin-bottom: 0.5rem;
			line-height: 0;

			width: 100%;
			max-width: 60%;
			max-height: 50%;
		}

		& > :global(picture > img) {
			object-fit: contain;

			width: 100%;
			height: 100%;
		}

		&.emote-only > :global(picture) {
			max-width: 100%;
			max-height: 100%;
			margin: 0;
		}
	}

	.name {
		font-weight: 500;
		max-width: 90%;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.user {
		font-size: 0.75rem;
		font-weight: 500;
		max-width: 90%;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.flags {
		position: absolute;
		top: 0.5rem;
		right: 0.5rem;

		display: flex;
		flex-direction: column;
		gap: 0.4rem;
		align-items: center;
	}
</style>
