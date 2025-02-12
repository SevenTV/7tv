<script lang="ts">
	import type {
		Emote,
		EmoteSearchResult,
		EmoteSetEmote,
		EmoteSetEmoteSearchResult,
	} from "$/gql/graphql";
	import { emotesLayout } from "$/lib/layout";
	import EmotePreview from "../emote-preview.svelte";
	import EmoteContainer from "./emote-container.svelte";
	import InfiniteLoading, { type InfiniteEvent } from "svelte-infinite-loading";
	import Spinner from "../spinner.svelte";
	import { untrack } from "svelte";
	import { defaultEmoteSet } from "$/lib/defaultEmoteSet";

	const PER_PAGE = 72;

	interface Props {
		load: (page: number, perPage: number) => Promise<EmoteSearchResult | EmoteSetEmoteSearchResult>;
		scrollable?: boolean;
		selectionMode?: boolean;
		selectionMap?: { [key: string]: boolean };
	}

	let { load, scrollable, selectionMode = false, selectionMap = $bindable({}) }: Props = $props();

	interface Results {
		items: { emote: Emote; emoteSetEmote?: EmoteSetEmote }[];
		pageCount: number;
		totalCount: number;
	}

	let page = $state(0);
	let results: Results | undefined = $state();

	let identifier = $state(0);

	export function reset() {
		page = 0;
		results = undefined;
		identifier++;
	}

	// Reset when the layout or the default emote set changes
	$effect(() => {
		// eslint-disable-next-line @typescript-eslint/no-unused-expressions
		$emotesLayout;
		// eslint-disable-next-line @typescript-eslint/no-unused-expressions
		$defaultEmoteSet;

		untrack(reset);
	});

	function handleInfinite(event: InfiniteEvent) {
		const currentIdentifier = identifier;

		load(++page, PER_PAGE)
			.then((result) => {
				if (currentIdentifier !== identifier) {
					return;
				}

				if (result.__typename === "EmoteSetEmoteSearchResult") {
					const items = result.items
						.filter((e) => e.emote)
						.map((item) => ({
							emote: item.emote!,
							emoteSetEmote: item,
						}));

					if (results) {
						results.pageCount = result.pageCount;
						results.totalCount = result.totalCount;
						results.items.push(...items);
					} else {
						results = {
							items,
							pageCount: result.pageCount,
							totalCount: result.totalCount,
						};
					}
				} else {
					result = result as EmoteSearchResult;

					const items = result.items.map((item) => ({
						emote: item,
						emoteSetEmote: undefined,
					}));

					if (results) {
						results.pageCount = result.pageCount;
						results.totalCount = result.totalCount;
						results.items.push(...items);
					} else {
						results = {
							items,
							pageCount: result.pageCount,
							totalCount: result.totalCount,
						};
					}
				}

				if (results.items.length > 0) {
					event.detail.loaded();
				}

				if (results.pageCount <= page) {
					event.detail.complete();
				}

				for (const item of results.items) {
					selectionMap[item.emote.id] = selectionMap[item.emote.id] || false;
				}
			})
			.catch(() => {
				event.detail.error();
			});
	}
</script>

<EmoteContainer {scrollable} layout={$emotesLayout} style="flex-grow: 1">
	{#if results}
		{#each results.items as data, i}
			<EmotePreview
				data={data.emote}
				emoteSetEmote={data.emoteSetEmote}
				index={i}
				emoteOnly={$emotesLayout === "small-grid"}
				{selectionMode}
				selected={selectionMap[data.emote.id]}
			/>
		{/each}
	{/if}
	<div class="loading">
		<!-- Still uses old Svelte 4 slots -->
		<InfiniteLoading
			distance={500}
			identifier={{
				identifier,
				layout: $emotesLayout,
			}}
			on:infinite={handleInfinite}
		>
			<p slot="noMore">No more emotes</p>
			<p slot="noResults">No emotes</p>
			<Spinner slot="spinner" />
		</InfiniteLoading>
	</div>
</EmoteContainer>

<style lang="scss">
	.loading {
		grid-column: 1 / -1;
		align-self: start;

		width: 100%;
		height: 1rem;
	}
</style>
