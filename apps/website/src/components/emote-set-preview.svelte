<script lang="ts">
	import { type EmoteSet } from "$/gql/graphql";
	import { user } from "$/lib/auth";
	import { defaultEmoteSet } from "$/lib/defaultEmoteSet";
	import Flags, { determineHighlightColor, emoteSetToFlags } from "./flags.svelte";
	import ResponsiveImage from "./responsive-image.svelte";

	type Props = {
		data: EmoteSet;
		bg?: "medium" | "light";
	};

	let { data, bg = "medium" }: Props = $props();

	let flags = $derived(emoteSetToFlags(data, $user, $defaultEmoteSet));

	let highlight = $derived(determineHighlightColor(flags));

	let usage = $derived.by(() => {
		if (!data.capacity) return undefined;

		if (data.emotes.totalCount === 0) return 0;

		return Math.round((data.emotes.totalCount / data.capacity) * 100);
	});

	let emotePreviews = $derived(
		data.emotes.items.filter((emote) => emote.emote).map((emote) => emote.emote!),
	);

	let placeholderCount = $derived.by(() => {
		let slots = 12;
		if (data.capacity) {
			slots = Math.min(data.capacity, slots);
		}

		return Math.max(slots - emotePreviews.length, 0);
	});
</script>

<a
	href="/emote-sets/{data.id}"
	class="emote-set"
	style={highlight
		? `--highlight: ${highlight}80; --highlight-active: ${highlight};`
		: "--highlight: transparent; --highlight-active: var(--border-active);"}
	style:background-color="var(--bg-{bg})"
>
	<div class="emotes" style:grid-template-columns="repeat({Math.min(data.capacity ?? 6, 6)}, 1fr)">
		{#each emotePreviews as emote, i}
			<ResponsiveImage
				images={emote.images}
				width={2 * 16}
				index={i}
				style="max-height: 2rem; overflow: hidden"
			/>
		{/each}
		<!-- Fill remaining slots (if any) with placeholders -->
		{#each Array(placeholderCount) as _}
			<div class="placeholder"></div>
		{/each}
	</div>
	<span class="name" style:color={highlight}>{data.name}</span>
	<Flags {flags} iconOnly style="justify-content: flex-end" />
	{#if usage !== undefined}
		<div class="usage" title="{data.emotes.totalCount} / {data.capacity}">
			<span class="text">{usage}%</span>
			<progress class="percentage" value={data.emotes.totalCount} max={data.capacity}></progress>
		</div>
	{/if}
</a>

<style lang="scss">
	.emote-set {
		color: var(--text);
		text-decoration: none;

		display: grid;
		align-items: start;
		grid-template-rows: 1fr auto auto;
		gap: 0.5rem;

		padding: 1rem;
		border: 1px solid transparent;
		border-radius: 0.75rem;
		cursor: pointer;

		border-color: var(--highlight);

		&:hover,
		&:focus-visible {
			border-color: var(--highlight-active);
		}
	}

	.emotes {
		grid-column: span 2;
		align-self: stretch;
		margin-bottom: 0.5rem;

		display: grid;
		justify-items: center;
		align-items: center;
		gap: 0.5rem;

		& > .placeholder {
			width: 2rem;
			height: 2rem;
			background-color: var(--preview);
			border-radius: 0.25rem;
		}
	}

	.name {
		font-size: 0.875rem;
		font-weight: 600;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.usage {
		position: relative;

		grid-column: span 2;

		display: flex;
		justify-content: center;

		& > progress[value] {
			position: absolute;
			width: 100%;
			height: 100%;

			background-color: var(--bg-dark);

			&::-webkit-progress-bar {
				background-color: var(--bg-dark);
			}

			&::-moz-progress-bar {
				background-color: var(--secondary);
			}

			&::-webkit-progress-value {
				background-color: var(--secondary);
			}
		}

		& > span {
			z-index: 1;
			padding: 0.3rem;

			text-align: center;
			font-size: 0.75rem;
			font-weight: 500;
		}
	}
</style>
