<script lang="ts">
	import EmoteSetPreview from "$/components/emote-set-preview.svelte";
	import Spinner from "$/components/spinner.svelte";
	import type { PageData } from "./$types";
	import Button from "$/components/input/button.svelte";
	import { Plus } from "phosphor-svelte";
	import CreateEmoteSetDialog from "$/components/dialogs/create-emote-set-dialog.svelte";
	import type { DialogMode } from "$/components/dialogs/dialog.svelte";
	import { user } from "$/lib/auth";
	import { UserEditorState } from "$/gql/graphql";
	import ActiveEmoteSetButton from "$/components/users/active-emote-set-button.svelte";
	import { t } from "svelte-i18n";

	let { data }: { data: PageData } = $props();

	let createEmoteSetDialog: DialogMode = $state("hidden");
</script>

<svelte:head>
	<title>Emote Sets - {$t("page_titles.suffix")}</title>
</svelte:head>

<div class="header-container">
	<h2>Emote Sets</h2>
</div>

<div class="buttons">
	<CreateEmoteSetDialog ownerId={data.id} bind:mode={createEmoteSetDialog} />
	<ActiveEmoteSetButton bind:userData={data.streamed.userRequest.value} />
	{#await data.streamed.userRequest.value}
		<Button primary disabled>
			{#snippet icon()}
				<Plus />
			{/snippet}
			<Spinner />
		</Button>
	{:then userData}
		{#if $user?.permissions.emoteSet.manage && ($user.permissions.emoteSet.manageAny || $user?.id === data.id || userData.editors.some((editor) => editor.editorId === $user?.id && editor.state === UserEditorState.Accepted && editor.permissions.emoteSet.create))}
			<Button primary onclick={() => (createEmoteSetDialog = "shown")}>
				{#snippet icon()}
					<Plus />
				{/snippet}
				Create New Set
			</Button>
		{/if}
	{/await}
</div>
{#await data.streamed.sets}
	<div class="spinner-wrapper">
		<Spinner />
	</div>
{:then sets}
	<div class="emote-sets">
		{#each sets as set}
			<EmoteSetPreview data={set} />
		{/each}
	</div>
{/await}

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

	.spinner-wrapper {
		margin: 0 auto;
	}

	.emote-sets {
		display: grid;
		gap: 1rem;
		grid-template-columns: repeat(auto-fill, minmax(17rem, 1fr));
	}
</style>
