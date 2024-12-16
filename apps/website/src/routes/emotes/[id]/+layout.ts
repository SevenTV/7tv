import { graphql } from "$/gql";
import type { LayoutLoadEvent } from "./$types";
import type { Emote } from "$/gql/graphql";
import { gqlClient } from "$/lib/gql";
import { get } from "svelte/store";
import { defaultEmoteSet } from "$/lib/defaultEmoteSet";

export async function load({ depends, fetch, params }: LayoutLoadEvent) {
	depends(`emotes:${params.id}`);

	const defaultSet = get(defaultEmoteSet);

	const req = gqlClient()
		.query(
			graphql(`
				query OneEmote($id: Id!, $isDefaultSetSet: Boolean!, $defaultSetId: Id!) {
					emotes {
						emote(id: $id) {
							id
							defaultName
							owner {
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
								editors {
									editorId
									permissions {
										emote {
											manage
										}
									}
								}
							}
							tags
							flags {
								animated
								approvedPersonal
								defaultZeroWidth
								deniedPersonal
								nsfw
								private
								publicListed
							}
							attribution {
								user {
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
									}
									highestRoleColor {
										hex
									}
								}
							}
							imagesPending
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
							deleted
						}
					}
				}
			`),
			{
				id: params.id,
				isDefaultSetSet: !!defaultSet,
				defaultSetId: defaultSet ?? "",
			},
			{
				fetch,
				requestPolicy: "network-only",
			},
		)
		.toPromise()
		.then((res) => res.data?.emotes.emote as Emote);

	return {
		id: params.id,
		streamed: {
			emote: req,
		},
	};
}
