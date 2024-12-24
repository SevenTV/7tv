<script lang="ts">
	import EmoteLoader from "$/components/layout/emote-loader.svelte";
	import { graphql } from "$/gql";
	import type { PageData } from "./$types";
	import type { EmoteSetEmoteSearchResult } from "$/gql/graphql";
	import { gqlClient } from "$/lib/gql";
	import LayoutButtons from "$/components/emotes/layout-buttons.svelte";
	import { emotesLayout } from "$/lib/layout";
	import { defaultEmoteSet } from "$/lib/defaultEmoteSet";
	import ActiveEmoteSetButton from "$/components/users/active-emote-set-button.svelte";
	import { t } from "svelte-i18n";

	let { data }: { data: PageData } = $props();

	function load(page: number, _perPage: number): Promise<EmoteSetEmoteSearchResult> {
		return gqlClient()
			.query(
				graphql(`
					query UserActiveEmotes(
						$id: Id!
						$page: Int!
						$isDefaultSetSet: Boolean!
						$defaultSetId: Id!
					) {
						users {
							user(id: $id) {
								style {
									activeEmoteSet {
										emotes(page: $page, perPage: 100) {
											__typename
											items {
												alias
												flags {
													zeroWidth
												}
												emote {
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
											totalCount
											pageCount
										}
									}
								}
							}
						}
					}
				`),
				{
					id: data.id,
					page,
					isDefaultSetSet: !!$defaultEmoteSet,
					defaultSetId: $defaultEmoteSet ?? "",
				},
			)
			.toPromise()
			.then((res) => {
				if (res.error || !res.data) {
					throw res.error;
				}

				const emotes = res.data.users.user?.style.activeEmoteSet?.emotes;

				if (!emotes) {
					return {
						items: [],
						totalCount: 0,
						pageCount: 0,
					};
				}

				return emotes as EmoteSetEmoteSearchResult;
			});
	}
</script>

<svelte:head>
	<title>Active Emotes - {$t("page_titles.suffix")}</title>
</svelte:head>

<div class="header-container">
	<h2>Active Emotes</h2>
</div>

<div class="buttons">
	<ActiveEmoteSetButton bind:userData={data.streamed.userRequest.value} />
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
