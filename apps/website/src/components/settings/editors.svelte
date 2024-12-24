<script lang="ts">
	import TextInput from "$/components/input/text-input.svelte";
	import { Check, MagnifyingGlass, Prohibit, Trash, UserCirclePlus } from "phosphor-svelte";
	import HideOn from "../hide-on.svelte";
	import { t } from "svelte-i18n";
	import {
		UserEditorState,
		UserEditorUpdateState,
		type UserEditor,
		type UserEditorPermissions,
		type UserEditorPermissionsInput,
		type UserSearchResult,
	} from "$/gql/graphql";
	import Date from "../date.svelte";
	import Flags, { editorPermissionsToFlags, editorStateToFlags } from "../flags.svelte";
	import Button from "../input/button.svelte";
	import moment from "moment";
	import UserProfilePicture from "../user-profile-picture.svelte";
	import UserName from "../user-name.svelte";
	import { pendingEditorFor, user } from "$/lib/auth";
	import { gqlClient } from "$/lib/gql";
	import { graphql } from "$/gql";
	import Spinner from "../spinner.svelte";
	import ChannelPreview from "../channel-preview.svelte";
	import EditorPermissionsDialog from "../dialogs/editor-permissions-dialog.svelte";
	import type { Snippet } from "svelte";

	interface Props {
		userId: string;
		editors: UserEditor[];
		tab: "editors" | "editing-for";
		children?: Snippet;
	}

	let { userId, editors = $bindable(), tab, children }: Props = $props();

	let query = $state("");

	let timeout: NodeJS.Timeout | number | undefined; // not reactive

	async function search(query: string): Promise<UserSearchResult> {
		if (!query) {
			return { items: [], totalCount: 0, pageCount: 0 };
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
							query EditorSearch($query: String!) {
								users {
									search(query: $query, page: 1, perPage: 5) {
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
								}
							}
						`),
						{ query },
					)
					.toPromise();

				if (res.error || !res.data) {
					reject();
					return;
				}

				resolve(res.data.users.search as UserSearchResult);
			}, 200);
		});
	}

	let results = $derived(search(query));

	async function addEditor(
		userId: string,
		editorId: string,
		permissions: UserEditorPermissionsInput,
	) {
		const res = await gqlClient().mutation(
			graphql(`
				mutation AddEditor(
					$userId: Id!
					$editorId: Id!
					$permissions: UserEditorPermissionsInput!
				) {
					userEditors {
						create(userId: $userId, editorId: $editorId, permissions: $permissions) {
							userId
							editorId
							editor {
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
							state
							permissions {
								superAdmin
								emoteSet {
									admin
									create
									manage
								}
								emote {
									admin
									create
									manage
									transfer
								}
								user {
									admin
									manageBilling
									manageEditors
									managePersonalEmoteSet
									manageProfile
								}
							}
							updatedAt
						}
					}
				}
			`),
			{ userId, editorId, permissions },
		);

		if (res.data?.userEditors.create) {
			editors = editors.concat(res.data.userEditors.create as UserEditor);
		}

		return res;
	}

	let adding = $state<{ userId: string; editorId: string }>();
	let editing = $state<{
		userId: string;
		editorId: string;
		permissions: UserEditorPermissionsInput;
	}>();

	function channelClick(e: MouseEvent, editorId: string) {
		e.preventDefault();

		adding = {
			userId,
			editorId,
		};
	}

	function startEditing(userId: string, editorId: string, initPerms: UserEditorPermissionsInput) {
		editing = {
			userId,
			editorId,
			permissions: initPerms,
		};
	}

	async function submitAdd(perms: UserEditorPermissionsInput) {
		if (adding) {
			await addEditor(adding.userId, adding.editorId, perms);
		}
	}

	async function submitEdit(perms: UserEditorPermissionsInput) {
		if (editing) {
			await updatePermissions(editing.userId, editing.editorId, perms);
		}
	}

	async function updatePermissions(
		userId: string,
		editorId: string,
		permissions: UserEditorPermissionsInput,
	) {
		const res = await gqlClient().mutation(
			graphql(`
				mutation UpdateEditorPermissions(
					$userId: Id!
					$editorId: Id!
					$permissions: UserEditorPermissionsInput!
				) {
					userEditors {
						editor(userId: $userId, editorId: $editorId) {
							updatePermissions(permissions: $permissions) {
								userId
								editorId
								permissions {
									superAdmin
									emoteSet {
										admin
										create
										manage
									}
									emote {
										admin
										create
										manage
										transfer
									}
									user {
										admin
										manageBilling
										manageEditors
										managePersonalEmoteSet
										manageProfile
									}
								}
							}
						}
					}
				}
			`),
			{ userId, editorId, permissions },
		);

		if (res.data?.userEditors.editor.updatePermissions) {
			editors = editors.map((editor) => {
				if (editor.userId === userId && editor.editorId === editorId) {
					editor.permissions = res.data!.userEditors.editor.updatePermissions
						.permissions as UserEditorPermissions;
				}

				return editor;
			});
		}
	}

	let deleteLoading = $state<{ userId: string; editorId: string }>();

	async function deleteEditor(userId: string, editorId: string) {
		deleteLoading = {
			userId,
			editorId,
		};

		const res = await gqlClient().mutation(
			graphql(`
				mutation DeleteEditor($userId: Id!, $editorId: Id!) {
					userEditors {
						editor(userId: $userId, editorId: $editorId) {
							delete
						}
					}
				}
			`),
			{ userId, editorId },
		);

		if (res.data?.userEditors.editor.delete) {
			editors = editors.filter(
				(editor) => editor.userId !== userId || editor.editorId !== editorId,
			);
		}

		deleteLoading = undefined;

		return res;
	}

	let acceptLoading = $state<{ userId: string; editorId: string }>();
	let rejectLoading = $state<{ userId: string; editorId: string }>();

	async function updateState(userId: string, editorId: string, state: UserEditorUpdateState) {
		if (state === UserEditorUpdateState.Accept) {
			acceptLoading = {
				userId,
				editorId,
			};
		} else if (state === UserEditorUpdateState.Reject) {
			rejectLoading = {
				userId,
				editorId,
			};
		}

		const res = await gqlClient().mutation(
			graphql(`
				mutation UpdateEditorState($userId: Id!, $editorId: Id!, $state: UserEditorUpdateState!) {
					userEditors {
						editor(userId: $userId, editorId: $editorId) {
							updateState(state: $state) {
								userId
								editorId
								state
							}
						}
					}
				}
			`),
			{ userId, editorId, state },
		);

		if (res.data?.userEditors.editor.updateState) {
			editors = editors.map((editor) => {
				if (editor.userId === userId && editor.editorId === editorId) {
					editor.state = res.data!.userEditors.editor.updateState.state;
				}

				return editor;
			});

			pendingEditorFor.update((c) => c - 1);
		}

		if (state === UserEditorUpdateState.Accept) {
			acceptLoading = undefined;
		} else if (state === UserEditorUpdateState.Reject) {
			rejectLoading = undefined;
		}

		return res;
	}

	let meId = $derived($user?.id);
</script>

<EditorPermissionsDialog
	bind:mode={() => (adding || editing ? "shown" : "hidden"),
	(mode) => {
		if (mode === "hidden") {
			adding = undefined;
			editing = undefined;
		}
	}}
	permissions={editing?.permissions}
	submit={adding ? submitAdd : submitEdit}
/>
<nav class="nav-bar">
	{@render children?.()}
	{#if tab === "editors"}
		<TextInput
			placeholder={$t("pages.settings.editors.add_editor")}
			bind:value={query}
			disabled={!!adding}
		>
			{#snippet icon()}
				{#await results}
					<Spinner />
				{:then _}
					<UserCirclePlus />
				{/await}
			{/snippet}
			{#snippet nonLabelChildren()}
				{#await results then results}
					{#if results && results.items.length > 0}
						<div class="results">
							{#each results.items as result}
								<ChannelPreview
									user={result}
									size={2}
									onclick={(e) => channelClick(e, result.id)}
								/>
							{/each}
						</div>
					{/if}
				{/await}
			{/snippet}
		</TextInput>
	{/if}
	<HideOn mobile>
		<TextInput placeholder={$t("labels.search")}>
			{#snippet icon()}
				<MagnifyingGlass />
			{/snippet}
		</TextInput>
	</HideOn>
</nav>

<div class="scroll">
	<table>
		<thead>
			<tr>
				<th>{$t("pages.settings.user_table.name")}</th>
				<th>{$t("pages.settings.user_table.last_modified")}</th>
				<th>{$t("pages.settings.user_table.permissions")}</th>
				<th></th>
			</tr>
		</thead>
		<tbody>
			{#if editors.length === 0}
				<tr class="data-row">
					<td class="shrink" colspan="4">
						<div class="no-data-container">
							<span class="no-data">No editors to show</span>
						</div>
					</td>
				</tr>
			{/if}
			{#each editors as editor}
				{@const user = tab === "editors" ? editor.editor : editor.user}
				{@const canEdit = tab === "editors" && meId !== editor.editorId}
				{#if user}
					<tr class="data-row">
						<td>
							<a class="user-info" href="/users/{user.id}">
								<UserProfilePicture {user} size={2.5 * 16} />
								<UserName {user} />
							</a>
						</td>
						<td class="date">
							<Date date={moment(editor.updatedAt)} />
						</td>
						<td>
							<Flags
								flags={editorPermissionsToFlags(editor.permissions)}
								edit={canEdit
									? () => startEditing(userId, editor.editorId, editor.permissions)
									: undefined}
							/>
						</td>
						<td class="shrink">
							<div class="buttons">
								<Flags flags={editorStateToFlags(editor.state, tab === "editing-for")} />
								{#if tab === "editing-for" && editor.state === UserEditorState.Pending}
									{@const aLoading =
										acceptLoading &&
										acceptLoading.userId === editor.userId &&
										acceptLoading.editorId === editor.editorId}
									{@const rLoading =
										rejectLoading &&
										rejectLoading.userId === editor.userId &&
										rejectLoading.editorId === editor.editorId}
									<Button
										disabled={aLoading}
										onclick={() =>
											updateState(editor.userId, editor.editorId, UserEditorUpdateState.Accept)}
									>
										{#snippet icon()}
											{#if aLoading}
												<Spinner />
											{:else}
												<Check />
											{/if}
										{/snippet}
									</Button>
									<Button
										disabled={rLoading}
										onclick={() =>
											updateState(editor.userId, editor.editorId, UserEditorUpdateState.Reject)}
									>
										{#snippet icon()}
											{#if rLoading}
												<Spinner />
											{:else}
												<Prohibit />
											{/if}
										{/snippet}
									</Button>
								{/if}
								{#if tab === "editing-for" || editor.state !== UserEditorState.Rejected}
									{@const loading =
										deleteLoading &&
										deleteLoading.userId === editor.userId &&
										deleteLoading.editorId === editor.editorId}
									<Button
										onclick={() => deleteEditor(editor.userId, editor.editorId)}
										disabled={loading}
									>
										{#snippet icon()}
											{#if loading}
												<Spinner />
											{:else}
												<Trash />
											{/if}
										{/snippet}
									</Button>
								{/if}
							</div>
						</td>
					</tr>
				{/if}
			{/each}
		</tbody>
	</table>
</div>

<style lang="scss">
	.nav-bar {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 1rem;
	}

	:global(label.input:has(input:enabled)):focus-within > .results {
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
	}

	@keyframes expand-down {
		from {
			height: 2rem;
		}
		to {
			height: 2.75rem;
		}
	}

	.scroll {
		scrollbar-gutter: stable;
	}

	.user-info {
		display: flex;
		align-items: center;
		gap: 1rem;

		color: var(--text);
		text-decoration: none;
	}

	.date {
		color: var(--text-light);
		font-size: 0.875rem;
	}

	.buttons {
		display: flex;
		align-items: center;
		justify-content: flex-end;
		gap: 0.5rem;
	}

	.no-data-container {
		text-align: center;
		padding: 0.5rem;
	}
</style>
