<script lang="ts">
	import Role from "$/components/users/role.svelte";
	import type { LayoutData } from "./$types";
	import TabLink from "$/components/tab-link.svelte";
	import {
		CaretDown,
		CaretRight,
		FolderSimple,
		Gear,
		IdentificationCard,
		Lightning,
		Link,
		PaintBrush,
		PencilSimple,
		Pulse,
		Upload,
		UserCircle,
	} from "phosphor-svelte";
	import Button from "$/components/input/button.svelte";
	import { t } from "svelte-i18n";
	import UserProfilePicture from "$/components/user-profile-picture.svelte";
	import Connections from "$/components/users/connections.svelte";
	import ChannelPreview from "$/components/channel-preview.svelte";
	import { UserEditorState } from "$/gql/graphql";
	import type { Snippet } from "svelte";
	import UserName from "$/components/user-name.svelte";
	import { user } from "$/lib/auth";
	import Badge from "$/components/badge.svelte";
	import { filterRoles } from "$/lib/utils";
	import { page } from "$app/stores";
	import Error from "$/components/error.svelte";

	let { data, children }: { data: LayoutData; children: Snippet } = $props();

	let connectionsExpanded = $state(false);
	let editorsExpanded = $state($page.url.pathname.endsWith("/editors"));

	let isMe = $derived(data.id === $user?.id);
	let canManageEditors = $derived(
		$user &&
			data.streamed.userRequest.value.then(
				(data) =>
					$user?.id === data.id ||
					$user.permissions.user.manageAny ||
					data.editors.some(
						(editor) =>
							editor.editor?.id === $user?.id &&
							editor.state === UserEditorState.Accepted &&
							editor.permissions.user.manageEditors,
					),
			),
	);

	let error = $state<string>();

	$effect(() => {
		data.streamed.userRequest.value
			.then((user) => {
				if (user && isMe && user.editors.length === 0) {
					editorsExpanded = true;
				}
			})
			.catch((e) => {
				error = e;
			});
	});
</script>

<svelte:head>
	{#await data.streamed.userRequest.value}
		<title>Loading... - {$t("page_titles.suffix")}</title>
	{:then userData}
		<title>{userData?.mainConnection?.platformDisplayName} - {$t("page_titles.suffix")}</title>
	{:catch}
		<title>Error - {$t("page_titles.suffix")}</title>
	{/await}
</svelte:head>

{#snippet desktopMenu()}
	<nav class="link-list hide-on-mobile">
		<TabLink title={$t("pages.user.active_emotes")} href="/users/{data.id}" big>
			<Lightning />
			{#snippet active()}
				<Lightning weight="fill" />
			{/snippet}
		</TabLink>
		<TabLink title={$t("pages.user.uploaded_emotes")} href="/users/{data.id}/uploaded" big>
			<Upload />
			{#snippet active()}
				<Upload weight="fill" />
			{/snippet}
		</TabLink>
		<TabLink
			title={$t("common.emote_sets", { values: { count: 2 } })}
			href="/users/{data.id}/emote-sets"
			big
		>
			<FolderSimple />
			{#snippet active()}
				<FolderSimple weight="fill" />
			{/snippet}
		</TabLink>
		<TabLink title={$t("common.activity")} href="/users/{data.id}/activity" big>
			<Pulse />
			{#snippet active()}
				<Pulse weight="fill" />
			{/snippet}
		</TabLink>
		<hr />
		<TabLink title={$t("common.cosmetics")} href="/users/{data.id}/cosmetics" big>
			<PaintBrush />
			{#snippet active()}
				<PaintBrush weight="fill" />
			{/snippet}
		</TabLink>
		{#await data.streamed.userRequest.value}
			<Button big onclick={() => (editorsExpanded = !editorsExpanded)}>
				{#snippet icon()}
					<UserCircle />
				{/snippet}
				{$t("common.editors")}
				{#snippet iconRight()}
					{#if editorsExpanded}
						<CaretDown style="margin-left: auto" />
					{:else}
						<CaretRight style="margin-left: auto" />
					{/if}
				{/snippet}
			</Button>
			<Button big onclick={() => (connectionsExpanded = !connectionsExpanded)}>
				{#snippet icon()}
					<Link />
				{/snippet}
				{$t("common.connections")}
				{#snippet iconRight()}
					{#if connectionsExpanded}
						<CaretDown style="margin-left: auto" />
					{:else}
						<CaretRight style="margin-left: auto" />
					{/if}
				{/snippet}
			</Button>
		{:then user}
			<Button big onclick={() => (editorsExpanded = !editorsExpanded)}>
				{#snippet icon()}
					<UserCircle />
				{/snippet}
				{$t("common.editors")}
				{#snippet iconRight()}
					{#if editorsExpanded}
						<CaretDown style="margin-left: auto" />
					{:else}
						<CaretRight style="margin-left: auto" />
					{/if}
				{/snippet}
			</Button>
			{#if editorsExpanded}
				<div class="expanded">
					{#each user.editors as editor}
						{#if editor.editor && editor.state === UserEditorState.Accepted}
							<ChannelPreview size={1.5} user={editor.editor} />
						{/if}
					{/each}
					{#await canManageEditors then manageEditors}
						{#if manageEditors}
							<TabLink title="Manage editors" href="/users/{data.id}/editors">
								<Gear />
								{#snippet active()}
									<Gear weight="fill" />
								{/snippet}
							</TabLink>
						{/if}
					{/await}
				</div>
			{/if}
			<Button big onclick={() => (connectionsExpanded = !connectionsExpanded)}>
				{#snippet icon()}
					<Link />
				{/snippet}
				{$t("common.connections")}
				{#snippet iconRight()}
					{#if connectionsExpanded}
						<CaretDown style="margin-left: auto" />
					{:else}
						<CaretRight style="margin-left: auto" />
					{/if}
				{/snippet}
			</Button>
			{#if connectionsExpanded}
				<div class="expanded">
					<Connections {user} />
					{#if isMe}
						<Button href="/settings/account">
							{#snippet icon()}
								<Gear />
							{/snippet}
							Manage connections
						</Button>
					{/if}
				</div>
			{/if}
		{/await}
	</nav>
{/snippet}

{#snippet mobileMenu()}
	<div class="header hide-on-desktop">
		<nav class="tabs">
			<TabLink title={$t("pages.user.about")} href="/users/{data.id}/about">
				<IdentificationCard />
				{#snippet active()}
					<IdentificationCard weight="fill" />
				{/snippet}
			</TabLink>
			{#await canManageEditors then manageEditors}
				{#if manageEditors}
					<TabLink title="Manage editors" href="/users/{data.id}/editors">
						<PencilSimple />
						{#snippet active()}
							<PencilSimple weight="fill" />
						{/snippet}
					</TabLink>
				{/if}
			{/await}
			<TabLink title={$t("common.active")} href="/users/{data.id}">
				<Lightning />
				{#snippet active()}
					<Lightning weight="fill" />
				{/snippet}
			</TabLink>
			<TabLink title={$t("pages.user.uploaded")} href="/users/{data.id}/uploaded">
				<Upload />
				{#snippet active()}
					<Upload weight="fill" />
				{/snippet}
			</TabLink>
			<TabLink
				title={$t("common.emote_sets", { values: { count: 2 } })}
				href="/users/{data.id}/emote-sets"
			>
				<FolderSimple />
				{#snippet active()}
					<FolderSimple weight="fill" />
				{/snippet}
			</TabLink>
			<TabLink title={$t("common.cosmetics")} href="/users/{data.id}/cosmetics">
				<PaintBrush />
				{#snippet active()}
					<PaintBrush weight="fill" />
				{/snippet}
			</TabLink>
			<TabLink title={$t("common.activity")} href="/users/{data.id}/activity">
				<Pulse />
				{#snippet active()}
					<Pulse weight="fill" />
				{/snippet}
			</TabLink>
		</nav>
	</div>
{/snippet}

{#snippet layout()}
	<div class="side-bar-layout">
		<aside class="side-bar">
			{#await data.streamed.userRequest.value}
				<UserProfilePicture
					user={undefined}
					size={4.75 * 16}
					style="align-self: center; grid-row: 1 / span 3; grid-column: 1;"
				/>
				<div class="name placeholder loading-animation"></div>
				<div class="roles placeholder loading-animation"></div>
			{:then user}
				<UserProfilePicture
					{user}
					size={4.75 * 16}
					style="align-self: center; grid-row: 1 / span 3; grid-column: 1;"
				/>
				<span class="name" style:color={user.highestRoleColor?.hex}>
					{#if user.style.activeBadge}
						<Badge badge={user.style.activeBadge} enableDialog />
					{/if}
					<UserName {user} enablePaintDialog />
				</span>
				<div class="roles">
					{#each filterRoles(user.roles) as role}
						<Role roleData={role} />
					{/each}
				</div>
			{/await}
			{@render desktopMenu()}
		</aside>
		<div class="content">
			{@render mobileMenu()}
			{@render children()}
		</div>
	</div>
{/snippet}

{#if error}
	<Error title="Failed to load user" details={error} />
{:else}
	{@render layout()}
{/if}

<style lang="scss">
	.side-bar {
		.placeholder {
			width: 100%;
			background-color: var(--preview);
			height: 22px;

			&.name {
				animation-delay: 0.1s;
			}

			&.roles {
				animation-delay: 0.2s;
			}
		}

		.name {
			align-self: center;
			max-width: 100%;

			display: flex;
			gap: 0.25rem;
			align-items: center;

			font-size: 1.125rem;
			font-weight: 600;
		}

		.roles {
			align-self: center;

			display: flex;
			gap: 0.25rem;
			flex-wrap: wrap;
		}

		// Select all buttons except the active one
		.link-list > :global(.button:not(.secondary)) {
			color: var(--text-light);
			font-weight: 500;
		}

		.expanded {
			margin-left: 0.5rem;

			display: flex;
			flex-direction: column;
			gap: 0.25rem;
		}
	}

	.content {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;

		.header {
			display: flex;
			align-items: center;
			justify-content: space-between;
			gap: 0.5rem;
		}

		.tabs {
			display: flex;
			border-radius: 0.5rem;
			background-color: var(--bg-light);
			overflow: auto;

			-ms-overflow-style: none;
			scrollbar-width: none;
			&::-webkit-scrollbar {
				display: none;
			}
		}
	}

	@media screen and (max-width: 960px) {
		.side-bar {
			display: grid;
			grid-template-columns: auto 1fr;
			grid-template-rows: auto auto auto auto;
			row-gap: 0.5rem;
			column-gap: 1rem;

			.name {
				grid-row: 1;
				grid-column: 2;
			}

			.roles {
				grid-row: 2;
				grid-column: 2;
			}
		}
	}
</style>
