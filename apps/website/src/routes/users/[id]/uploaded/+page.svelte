<script lang="ts">
	import EmoteLoader from "$/components/layout/emote-loader.svelte";
	import { graphql } from "$/gql";
	import type { PageData } from "./$types";
	import type { Emote, EmoteSearchResult } from "$/gql/graphql";
	import { gqlClient } from "$/lib/gql";
	import LayoutButtons from "$/components/emotes/layout-buttons.svelte";
	import { defaultEmoteSet } from "$/lib/defaultEmoteSet";
	import { emotesLayout } from "$/lib/layout";
	import { t } from "svelte-i18n";
	import Button from "$/components/input/button.svelte";
	import { uploadDialogMode } from "$/lib/layout";
	import { PlusSquare } from "phosphor-svelte";


	let { data }: { data: PageData } = $props();

	function load(_page: number, _perPage: number): Promise<EmoteSearchResult> {
		return gqlClient()
			.query(
				graphql(`
					query UserUploadedEmotes($id: Id!, $isDefaultSetSet: Boolean!, $defaultSetId: Id!) {
						users {
							user(id: $id) {
								ownedEmotes {
									id
									defaultName
									owner {
										mainConnection {
											platformDisplayName
										}
										style {
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
									flags {
										# animated
										# approvedPersonal
										defaultZeroWidth
										# deniedPersonal
										# nsfw
										# private
										publicListed
									}
									images {
										url
										mime
										size
										scale
										width
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
							}
						}
					}
				`),
				{
					id: data.id,
					isDefaultSetSet: !!$defaultEmoteSet,
					defaultSetId: $defaultEmoteSet ?? "",
				},
			)
			.toPromise()
			.then((res) => {
				if (res.error || !res.data) {
					throw res.error;
				}

				const emotes = res.data.users.user?.ownedEmotes;

				if (!emotes) {
					throw new Error("No emotes found");
				}

				return {
					items: emotes as Emote[],
					totalCount: emotes.length,
					pageCount: 1,
				};
			});
	}
</script>

<svelte:head>
	<title>Uploaded Emotes - {$t("page_titles.suffix")}</title>
</svelte:head>

<div class="header-container">
	<h2>Uploaded Emotes</h2>
</div>

<div class="buttons">
	<Button secondary onclick={() => ($uploadDialogMode = "shown")}>
		{#snippet icon()}
			<PlusSquare />
		{/snippet}
		{$t("dialogs.upload.upload")}
	</Button>
	<div class="layout-buttons">
		<LayoutButtons bind:value={$emotesLayout} />
	</div>
</div>
{#key data.id}
	<EmoteLoader {load} />
{/key}

<style lang="scss">
	.header-container {
		display: flex;
		justify-content: space-between;
		height: 40px;
		
		h2 {
			font-family: "AKONY";
			font-size: 1.5rem;
			font-weight: 700;
			margin: auto 0;
		}
	}

	.buttons {
		display: flex;
		gap: 0.5rem;
		align-items: center;
		justify-content: space-between;
	}

	.layout-buttons {
		display: flex;
		gap: 0.5rem;
	}
</style>
