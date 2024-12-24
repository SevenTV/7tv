<script lang="ts">
	import Tags from "$/components/emotes/tags.svelte";
	import Flags, { emoteSetToFlags } from "$/components/flags.svelte";
	import { type DialogMode } from "$/components/dialogs/dialog.svelte";
	import { t } from "svelte-i18n";
	import { gqlClient } from "$/lib/gql";
	import { graphql } from "$/gql";
	import EmoteLoader from "$/components/layout/emote-loader.svelte";
	import {
		EmoteSetKind,
		UserEditorState,
		type EmoteSet,
		type EmoteSetEmoteSearchResult,
	} from "$/gql/graphql";
	import Button from "$/components/input/button.svelte";
	import LayoutButtons from "$/components/emotes/layout-buttons.svelte";
	import { emotesLayout } from "$/lib/layout";
	import { defaultEmoteSet } from "$/lib/defaultEmoteSet";
	import { Lightning, LightningSlash, MagnifyingGlass, NotePencil } from "phosphor-svelte";
	import TextInput from "$/components/input/text-input.svelte";
	import { untrack } from "svelte";
	import { refreshUser, user } from "$/lib/auth";
	import { setActiveSet } from "$/lib/userMutations";
	import Spinner from "$/components/spinner.svelte";
	import EditEmoteSetDialog from "$/components/dialogs/edit-emote-set-dialog.svelte";

	let { data }: { data: EmoteSet } = $props();

	// let selectionMode = $state(false);
	// let selectionMap = $state({});
	let editDialogMode: DialogMode = $state("hidden");
	// let copyEmotesDialogMode: DialogMode = $state("hidden");
	// let removeEmotesDialogMode: DialogMode = $state("hidden");

	let query = $state("");

	let timeout: NodeJS.Timeout | number | undefined; // not reactive

	async function queryEmotes(
		query: string | undefined,
		page: number,
		perPage: number,
	): Promise<EmoteSetEmoteSearchResult> {
		// Small timeout to prevent spamming requests when user is typing
		return new Promise((resolve, reject) => {
			if (timeout) {
				clearTimeout(timeout);
			}

			timeout = setTimeout(async () => {
				const res = await gqlClient()
					.query(
						graphql(`
							query EmotesInSet(
								$id: Id!
								$query: String
								$page: Int!
								$perPage: Int!
								$isDefaultSetSet: Boolean!
								$defaultSetId: Id!
							) {
								emoteSets {
									emoteSet(id: $id) {
										emotes(query: $query, page: $page, perPage: $perPage) {
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
						`),
						{
							id: data.id,
							query: query,
							page,
							perPage,
							isDefaultSetSet: !!$defaultEmoteSet,
							defaultSetId: $defaultEmoteSet ?? "",
						},
					)
					.toPromise();

				if (res.error || !res.data) {
					reject(res.error);
					return;
				}

				const emotes = res.data.emoteSets.emoteSet?.emotes;

				if (!emotes) {
					reject(new Error("No emotes found"));
					return;
				}

				resolve(emotes as EmoteSetEmoteSearchResult);
			}, 200);
		});
	}

	let loader: ReturnType<typeof EmoteLoader>;

	$effect(() => {
		// eslint-disable-next-line @typescript-eslint/no-unused-expressions
		query; // trigger reactivity when query changes
		untrack(() => {
			loader?.reset();
		});
	});

	let setActiveLoading = $state(false);

	async function setAsActiveSet(id?: string) {
		if (!$user) {
			return;
		}

		setActiveLoading = true;

		await setActiveSet($user.id, id);

		refreshUser();

		setActiveLoading = false;
	}

	let isActive = $derived($user?.style.activeEmoteSetId === data.id);
</script>

<svelte:head>
	<title>{data.name} - {$t("page_titles.suffix")}</title>
</svelte:head>

<EditEmoteSetDialog bind:mode={editDialogMode} bind:data />
<!-- <CopyEmotesDialog bind:mode={copyEmotesDialogMode} />
<RemoveEmotesDialog bind:mode={removeEmotesDialogMode} /> -->
<div class="layout">
	<div class="set-info">
		<h1>{data.name}</h1>
		<Flags
			flags={emoteSetToFlags(data, $user, $defaultEmoteSet)}
			style="position: absolute; top: 1rem; right: 1rem;"
		/>
		<Tags tags={data.tags} />
		{#if data.capacity}
			<div class="progress">
				<progress value={data.emotes.totalCount} max={data.capacity}></progress>
				{data.emotes.totalCount}/{data.capacity}
			</div>
		{/if}
	</div>
	<div class="controls">
		<div class="buttons">
			<!-- <Button secondary onclick={() => (selectionMode = !selectionMode)} hideOnDesktop>
				{$t("labels.select")}
				{#snippet iconRight()}
					<Toggle bind:value={selectionMode} />
				{/snippet}
			</Button> -->
			{#if $user}
				{#snippet loadingSpinner()}
					<Spinner />
				{/snippet}
				<!-- <HideOn mobile={selectionMode}> -->
				{#if data.kind === EmoteSetKind.Normal && ($user.id === data.owner?.id || $user.permissions.user.manageAny || data.owner?.editors.some((editor) => editor?.editorId === $user?.id && editor.permissions.user.manageProfile))}
					<Button
						primary
						onclick={() => setAsActiveSet(isActive ? undefined : data.id)}
						icon={setActiveLoading ? loadingSpinner : undefined}
						disabled={setActiveLoading}
					>
						{#if isActive}
							{$t("labels.disable")}
						{:else}
							{$t("labels.enable")}
						{/if}
						{#snippet iconRight()}
							{#if isActive}
								<LightningSlash />
							{:else}
								<Lightning />
							{/if}
						{/snippet}
					</Button>
				{/if}
				{#if $user?.permissions.emoteSet.manage && ($user.id === data.owner?.id || $user.permissions.emoteSet.manageAny || data.owner?.editors.some((editor) => editor?.editorId === $user?.id && editor.state === UserEditorState.Accepted && editor.permissions.emoteSet.manage))}
					<Button secondary hideOnMobile onclick={() => (editDialogMode = "shown")}>
						{$t("labels.edit")}
						{#snippet iconRight()}
							<NotePencil />
						{/snippet}
					</Button>
					<Button secondary hideOnDesktop onclick={() => (editDialogMode = "shown")}>
						{#snippet iconRight()}
							<NotePencil />
						{/snippet}
					</Button>
				{/if}
				<!-- </HideOn> -->
			{/if}
			<!-- <Button secondary hideOnMobile>
				{$t("pages.emote_set.copy_set")}
				{#snippet iconRight()}
					<Copy />
				{/snippet}
			</Button>
			{#if !selectionMode}
				<Button secondary hideOnDesktop>
					{#snippet iconRight()}
						<Copy />
					{/snippet}
				</Button>
			{/if}
			<Button secondary onclick={() => (selectionMode = !selectionMode)} hideOnMobile>
				{$t("labels.selection_mode")}
				{#snippet iconRight()}
					<Toggle bind:value={selectionMode} />
				{/snippet}
			</Button>
			{#if selectionMode}
				<Button onclick={() => (copyEmotesDialogMode = "shown")}>
					{#snippet icon()}
						<Copy />
					{/snippet}
				</Button>
				<Button>
					{#snippet icon()}
						<NotePencil />
					{/snippet}
				</Button>
				<Button onclick={() => (removeEmotesDialogMode = "shown")}>
					{#snippet icon()}
						<Trash />
					{/snippet}
				</Button>
			{/if} -->
		</div>
		<div class="buttons">
			<!-- <Select
				options={[
					{ value: "none", label: $t("labels.no_filters") },
					{ value: "filters", label: $t("labels.filters") },
				]}
			/> -->
			<TextInput placeholder={$t("labels.search")} bind:value={query}>
				{#snippet icon()}
					<MagnifyingGlass />
				{/snippet}
			</TextInput>
			<LayoutButtons bind:value={$emotesLayout} />
		</div>
	</div>
	<div class="content">
		<!-- <EmoteLoader
			bind:this={loader}
			load={(page, perPage) => queryEmotes(query || undefined, page, perPage)}
			scrollable={false}
			{selectionMode}
			bind:selectionMap
		/> -->
		<EmoteLoader
			bind:this={loader}
			load={(page, perPage) => queryEmotes(query || undefined, page, perPage)}
			scrollable={false}
		/>
	</div>
</div>

<style lang="scss">
	.layout {
		padding: 1.25rem;
		padding-bottom: 0;
		height: 100%;

		display: flex;
		flex-direction: column;
		gap: 1rem;
	}

	.set-info {
		position: relative;
		padding: 1rem;

		display: flex;
		flex-direction: column;
		gap: 0.75rem;

		background-color: var(--bg-medium);
		border-radius: 0.5rem;

		h1 {
			text-align: center;
			font-size: 1.125rem;
			font-weight: 500;
		}

		.progress {
			display: flex;
			align-items: center;
			gap: 0.75rem;
			
			font-size: 0.875rem;
			font-weight: 500;
			
			progress {
				height: 0.875rem;
				flex-grow: 1;
			}
		}
	}

	.controls {
		display: flex;
		gap: 0.5rem;
		flex-wrap: wrap-reverse;
		justify-content: space-between;
	}

	.buttons {
		display: flex;
		gap: 0.5rem;
		align-items: center;
	}

	.content {
		flex-grow: 1;

		overflow: auto;
		scrollbar-gutter: stable;
		margin-right: -1.25rem;
		padding-right: 1.25rem;
	}

	@media screen and (max-width: 960px) {
		.layout {
			padding: 0.5rem;
			padding-bottom: 0;
		}

		.content {
			margin-right: 0;
			padding-right: 0;
		}
	}
</style>
