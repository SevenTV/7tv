<script lang="ts">
	import Logo from "$/components/icons/logo.svelte";
	import TopTabs from "./top-tabs.svelte";
	import HideOn from "../hide-on.svelte";
	import { showMobileMenu, signInDialogMode, uploadDialogMode } from "$/lib/layout";
	import { user } from "$/lib/auth";
	import DropDown from "../drop-down.svelte";
	import Menu from "./menu.svelte";
	import { List, MagnifyingGlass, PlusSquare } from "phosphor-svelte";
	import Button from "../input/button.svelte";
	import { t } from "svelte-i18n";
	import Spinner from "../spinner.svelte";
	import UserProfilePicture from "../user-profile-picture.svelte";
	import GlobalSearch from "./global-search.svelte";
	import UserName from "../user-name.svelte";
	import mouseTrap from "$/lib/mouseTrap";
	import { afterNavigate } from "$app/navigation";

	let mobileSearchShown = $state(false);

	let globalSearch: ReturnType<typeof GlobalSearch>;

	$effect(() => {
		if (mobileSearchShown) {
			globalSearch?.focus();
		}
	});

	afterNavigate(() => {
		mobileSearchShown = false;
	});

	let tabs = $derived.by(() => {
		const tabs = [
			{ name: $t("common.emotes", { values: { count: 2 } }), pathname: "/emotes" },
			{ name: $t("pages.store.title"), pathname: "/store", highlight: "var(--store)" },
		];

		if (
			$user?.permissions.user.manageAny ||
			$user?.permissions.admin.manageRedeemCodes ||
			$user?.permissions.admin.manageEntitlements
		) {
			tabs.push({ name: $t("pages.admin.title"), pathname: "/admin", highlight: "var(--staff)" });
		}

		return tabs;
	});
</script>

<nav>
	<div class="links">
		<a class="home" href="/" class:hide-on-mobile={mobileSearchShown}>
			<Logo />
		</a>
		<HideOn mobile>
			<TopTabs {tabs} />
		</HideOn>
	</div>
	<HideOn mobile={!mobileSearchShown}>
		<div use:mouseTrap={() => (mobileSearchShown = false)} style="display:contents">
			<GlobalSearch bind:this={globalSearch} />
		</div>
	</HideOn>
	<div class="user-actions">
		{#if !mobileSearchShown}
			<Button hideOnDesktop onclick={() => (mobileSearchShown = true)}>
				<MagnifyingGlass />
			</Button>
		{/if}
		{#if $user}
			<Button
				hideOnDesktop
				hideOnMobile={mobileSearchShown}
				onclick={() => ($uploadDialogMode = "shown")}
			>
				{#snippet icon()}
					<PlusSquare />
				{/snippet}
			</Button>
			<Button secondary hideOnMobile onclick={() => ($uploadDialogMode = "shown")}>
				{#snippet icon()}
					<PlusSquare />
				{/snippet}
				{$t("dialogs.upload.upload")}
			</Button>
		{:else if $user === undefined}
			<Spinner />
		{/if}

		{#if $user !== undefined}
			<HideOn mobile>
				<DropDown hover={true}>
					{#if $user}
						<Button class="profile">
							<UserProfilePicture user={$user} size={32} />
							<span class="profile-name">
								<UserName user={$user} />
							</span>
						</Button>
					{:else}
						<Button>
							{#snippet icon()}
								<List />
							{/snippet}
						</Button>
					{/if}
					{#snippet dropdown(close)}
						<Menu onCloseRequest={close} />
					{/snippet}
				</DropDown>
			</HideOn>
		{/if}

		{#if !mobileSearchShown}
			{#if $user}
				<button
					class="profile hide-on-desktop"
					onclick={() => ($showMobileMenu = !$showMobileMenu)}
				>
					<UserProfilePicture user={$user} size={32} />
				</button>
			{:else if $user === null}
				<Button primary onclick={() => ($signInDialogMode = "shown")}>
					{$t("common.sign_in")}
				</Button>
			{/if}
			<!-- Only show when logged out on mobile -->
			{#if $user === null}
				<Button hideOnDesktop onclick={() => ($showMobileMenu = !$showMobileMenu)}>
					{#snippet icon()}
						<List />
					{/snippet}
				</Button>
			{/if}
		{/if}
	</div>
</nav>

<style lang="scss">
	nav {
		background-color: var(--bg-dark);
		border-bottom: 1px solid var(--border-active);
		padding: 0 2rem;
		height: 3.5rem;

		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 1rem;
	}

	.links {
		/* Take all available space but shrink by a very high factor */
		flex: 1 9999;

		display: flex;
		gap: 2rem;

		.home {
			color: var(--text);

			display: flex;
			align-items: center;
		}
	}

	.user-actions {
		/* Take all available space but shrink by a very high factor */
		flex: 1 9999;

		display: flex;
		gap: 0.75rem;
		align-items: center;
		justify-content: flex-end;

		.profile {
			color: var(--text);

			display: flex;
			align-items: center;
			gap: 0.5rem;
			text-decoration: none;
		}

		.profile-name {
			font-weight: 600;
		}
	}

	@media screen and (max-width: 960px) {
		nav {
			padding: 0 1rem;
			gap: 1rem;
		}

		.links {
			gap: 1rem;
		}
	}
</style>
