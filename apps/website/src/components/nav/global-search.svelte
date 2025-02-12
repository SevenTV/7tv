<script lang="ts">
	import { MagnifyingGlass } from "phosphor-svelte";
	import TextInput from "../input/text-input.svelte";
	import { t } from "svelte-i18n";
	import { gqlClient } from "$/lib/gql";
	import { graphql } from "$/gql";
	import type { SearchResultAll } from "$/gql/graphql";
	import Spinner from "../spinner.svelte";
	import ResponsiveImage from "../responsive-image.svelte";
	import ChannelPreview from "../channel-preview.svelte";
	import Button from "../input/button.svelte";
	import Flags, { emoteToFlags } from "../flags.svelte";
	import { defaultEmoteSet } from "$/lib/defaultEmoteSet";
	import { editableEmoteSets } from "$/lib/emoteSets";
	import { user } from "$/lib/auth";
	import { goto } from "$app/navigation";

	let query = $state("");
	let timeout: NodeJS.Timeout | number | undefined; // not reactive
	let results = $derived(search(query));
	let input: ReturnType<typeof TextInput>;
	let resultSelectedIndex: number = -1;

	async function search(query: string): Promise<SearchResultAll> {
		if (!query) {
			const users = $user?.editorFor.map((editor) => editor.user).filter((u) => !!u) ?? [];

			return {
				users: { items: users, totalCount: users.length, pageCount: 1 },
				emotes: { items: [], totalCount: 0, pageCount: 0 },
			};
		}

		// Small timeout to prevent spamming requests when user is typing

		return new Promise((resolve, reject) => {
			if (timeout) {
				clearTimeout(timeout);
			}

			timeout = setTimeout(async () => {
				const res = await gqlClient()
					.query(
						graphql(`
							query GlobalSearch($query: String!, $isDefaultSetSet: Boolean!, $defaultSetId: Id!) {
								search {
									all(query: $query, page: 1, perPage: 5) {
										users {
											items {
												id
												mainConnection {
													platformDisplayName
													platformAvatarUrl
												}
												style {
													activeProfilePicture {
														images {
															url
															mime
															size
															width
															height
															scale
															frameCount
														}
													}
													activePaint {
														id
														name
														data {
															layers {
																id
																ty {
																	__typename
																	... on PaintLayerTypeSingleColor {
																		color {
																			hex
																		}
																	}
																	... on PaintLayerTypeLinearGradient {
																		angle
																		repeating
																		stops {
																			at
																			color {
																				hex
																			}
																		}
																	}
																	... on PaintLayerTypeRadialGradient {
																		repeating
																		stops {
																			at
																			color {
																				hex
																			}
																		}
																		shape
																	}
																	... on PaintLayerTypeImage {
																		images {
																			url
																			mime
																			size
																			scale
																			width
																			height
																			frameCount
																		}
																	}
																}
																opacity
															}
															shadows {
																color {
																	hex
																}
																offsetX
																offsetY
																blur
															}
														}
													}
												}
												highestRoleColor {
													hex
												}
											}
											totalCount
											pageCount
										}
										emotes {
											items {
												id
												defaultName
												flags {
													# animated
													# approvedPersonal
													defaultZeroWidth
													# deniedPersonal
													# nsfw
													private
													publicListed
												}
												images {
													url
													mime
													size
													width
													height
													scale
													frameCount
												}
												ranking(ranking: TRENDING_WEEKLY)
												inEmoteSets(emoteSetIds: [$defaultSetId]) @include(if: $isDefaultSetSet) {
													emoteSetId
													emote {
														id
														alias
													}
												}
											}
											totalCount
											pageCount
										}
									}
								}
							}
						`),
						{ query, isDefaultSetSet: !!$defaultEmoteSet, defaultSetId: $defaultEmoteSet ?? "" },
					)
					.toPromise();

				if (res.error || !res.data) {
					reject();
					return;
				}

				resolve(res.data.search.all as SearchResultAll);
			}, 200);
		});
	}

	export function focus() {
		input?.focus();
	}

	function onkeydown(event: KeyboardEvent) {
		// Handle Ctrl + K
		if (event.ctrlKey && event.key === "k") {
			input?.focus();
			event.preventDefault();
			event.stopPropagation();
			return;
		}
		// Handle Up and Down scrolling
		if (event.key === "ArrowUp" || event.key === "ArrowDown") {
			const resultsElement = document.getElementById("globalResults");
			if (!resultsElement || !resultsElement.checkVisibility()) {
				return;
			}

			const links = resultsElement.querySelectorAll("a");

			switch (event.key) {
				case "ArrowUp":
					resultSelectedIndex =
						resultSelectedIndex === -1 ? links.length - 1 : resultSelectedIndex - 1;
					break;

				case "ArrowDown":
					resultSelectedIndex = resultSelectedIndex === -1 ? 0 : resultSelectedIndex + 1;
					break;

				default:
					return resultSelectedIndex;
			}

			// Wrap around the index
			resultSelectedIndex = (resultSelectedIndex + links.length) % links.length;
			links[resultSelectedIndex].focus();

			event.preventDefault();
			event.stopPropagation();
			return;
		}
	}

	function handleInputKeyPress(event: KeyboardEvent) {
		if (event.key === "Enter" && query.trim() != "") {
			goto(`/emotes?q=${query}&updateSearch=true`);
		}
	}

	$effect(() => {
		// eslint-disable-next-line @typescript-eslint/no-unused-expressions
		query;
		resultSelectedIndex = -1;
	});
</script>

<svelte:window {onkeydown} />

<TextInput
	placeholder={$t("labels.search")}
	bind:value={query}
	style="flex: 0 1 20rem"
	onkeypress={handleInputKeyPress}
	big
	bind:this={input}
>
	{#snippet icon()}
		{#await results}
			<Spinner />
		{:then _}
			<MagnifyingGlass />
		{/await}
	{/snippet}
	{#snippet nonLabelChildren()}
		{#await results then results}
			{#if results && (results.users.items.length > 0 || results.emotes.items.length > 0)}
				<div id="globalResults" class="results">
					{#if results.emotes.items}
						<span class="label">Emotes</span>
					{/if}
					{#each results.emotes.items as result}
						<Button href="/emotes/{result.id}" class="item">
							{#snippet icon()}
								<ResponsiveImage images={result.images} width={16 * 2} />
							{/snippet}
							{result.defaultName}
							{#snippet iconRight()}
								<Flags
									flags={emoteToFlags(result, $defaultEmoteSet, $editableEmoteSets)}
									iconOnly
								/>
							{/snippet}
						</Button>
					{/each}
					{#if results.users.items && results.emotes.items}
						<hr />
					{/if}
					{#if results.users.items}
						<span class="label">Users</span>
					{/if}
					{#each results.users.items as result}
						<ChannelPreview user={result} size={2} />
					{/each}
				</div>
			{/if}
		{/await}
	{/snippet}
</TextInput>

<style lang="scss">
	:global(label.input):focus-within > .results {
		display: flex;
	}

	.results {
		position: absolute;
		top: calc(100% + 0.25rem);
		left: 0;
		right: 0;
		z-index: 10;

		background-color: var(--bg-light);

		border: 1px solid var(--border-active);
		border-radius: 0.5rem;

		display: none;
		overflow: hidden;

		flex-direction: column;

		& > :global(.button) {
			animation: expand-down 0.2s forwards;
		}

		.label {
			padding: 0.5rem;
			font-size: 1rem;
			color: var(--text-light);
		}
	}

	@keyframes expand-down {
		from {
			height: 2rem;
		}
		to {
			height: 2.75rem;
		}
	}
</style>
