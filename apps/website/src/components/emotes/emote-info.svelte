<script lang="ts">
	import {
		ArrowBendDownRight,
		FolderPlus,
		NotePencil,
		CaretDown,
		PaperPlaneRight,
		ArrowsMerge,
		Download,
		Trash,
		Flag,
		CaretRight,
		Cpu,
		CopySimple,
	} from "phosphor-svelte";
	import Tags from "$/components/emotes/tags.svelte";
	import Flags, { emoteToFlags } from "$/components/flags.svelte";
	import Button from "../input/button.svelte";
	import { t } from "svelte-i18n";
	import DropDown from "../drop-down.svelte";
	import AddEmoteDialog from "../dialogs/add-emote-dialog.svelte";
	import EditEmoteDialog from "../dialogs/edit-emote-dialog.svelte";
	import TransferEmoteDialog from "../dialogs/transfer-emote-dialog.svelte";
	import ReportEmoteDialog from "../dialogs/report-emote-dialog.svelte";
	import DeleteEmoteDialog from "../dialogs/delete-emote-dialog.svelte";
	import MenuButton from "../input/menu-button.svelte";
	import type { Emote, Image } from "$/gql/graphql";
	import EmoteInfoImage from "./emote-info-image.svelte";
	import { formatSortIndex } from "../responsive-image.svelte";
	import UserProfilePicture from "../user-profile-picture.svelte";
	import EmoteLoadingPlaceholder from "../emote-loading-placeholder.svelte";
	import type { DialogMode } from "../dialogs/dialog.svelte";
	import type { Snippet } from "svelte";
	import UserName from "../user-name.svelte";
	import { user } from "$/lib/auth";
	import { defaultEmoteSet } from "$/lib/defaultEmoteSet";
	import EmoteUseButton from "../emote-use-button.svelte";
	import { editableEmoteSets } from "$/lib/emoteSets";
	import Spinner from "../spinner.svelte";
	import { invalidate } from "$app/navigation";
	import { isSafari } from "$/lib/utils";

	type MoreMenuMode = "root" | "format" | "size";
	type MoreMenuAction = "download" | "copy";

	let { data, children }: { data: Emote | null; children?: Snippet } = $props();

	// svelte-ignore non_reactive_update
	let moreMenuDropdown: ReturnType<typeof DropDown>;
	let moreMenuMode: MoreMenuMode = $state("root");
	let downloadFormat = $state<string>();
	let moreMenuAction: MoreMenuAction = $state("download");

	let formats = $derived(
		data?.images
			.filter((image) => image.frameCount > 1 === data?.flags.animated)
			.reduce((acc, image) => {
				if (!acc.includes(image.mime)) {
					acc.push(image.mime);
				}
				return acc;
			}, [] as string[])
			.toSorted(),
	);
	let sizes = $derived(
		data?.images
			.filter((image) => image.mime === downloadFormat)
			.filter((image) => image.frameCount > 1 === data?.flags.animated)
			.reduce((acc, image) => {
				if (!acc[image.scale]) {
					acc[image.scale] = image;
				}
				return acc;
			}, [] as Image[])
			.filter((i) => !!i),
	);

	function clickFormat(format: string) {
		downloadFormat = format;
		moreMenuMode = "size";
	}

	let addEmoteDialogMode: DialogMode = $state("hidden");
	let editDialogMode: DialogMode = $state("hidden");
	let transferDialogMode: DialogMode = $state("hidden");
	let deleteDialogMode: DialogMode = $state("hidden");
	let reportDialogMode: DialogMode = $state("hidden");

	function prepareImages(images: Image[]): Image[][] {
		const safari = isSafari();

		const animated = images.some((i) => i.frameCount > 1);

		const result: Image[][] = [];

		for (let i = 0; i < images.length; i++) {
			const image = images[i];

			// Safari doesn't fully support animated AVIF
			if (safari && image.mime === "image/avif" && image.frameCount > 1) {
				continue;
			}

			if (animated && image.frameCount === 1) {
				continue;
			}

			if (!result[image.scale]) {
				result[image.scale] = [];
			}

			result[image.scale][formatSortIndex(image, true)] = image;
		}

		return result;
	}

	let editPermission = $derived(
		data && $user
			? data.owner?.id === $user.id ||
					$user.permissions.emote.manageAny ||
					data.owner?.editors.find((e) => e.editorId === $user.id)?.permissions.emote.manage
			: undefined,
	);

	// svelte-ignore non_reactive_update
	let downloadElement: HTMLAnchorElement;

	function downloadImage(image: Image) {
		if (!downloadElement) return;

		moreMenuDropdown?.close();

		fetch(image.url).then((response) => {
			response.blob().then((blob) => {
				const url = URL.createObjectURL(blob);
				downloadElement.href = url;
				downloadElement.download = `${data?.defaultName}-${image.scale}x`;
				downloadElement.click();
				URL.revokeObjectURL(url);
			});
		});
	}

	function setDialogState(mode: MoreMenuMode, download: boolean = true) {
		moreMenuMode = mode;
		moreMenuAction = download ? "download" : "copy";
	}

	async function copyToClipboard(text: string) {
		moreMenuDropdown?.close();
		await navigator.clipboard.writeText(text);
	}
</script>

{#if !children && data}
	<!-- Rerender when opened -->
	{#if addEmoteDialogMode !== "hidden"}
		<AddEmoteDialog bind:mode={addEmoteDialogMode} {data} />
	{/if}
	<EditEmoteDialog bind:mode={editDialogMode} bind:data />
	<TransferEmoteDialog bind:mode={transferDialogMode} bind:data />
	<ReportEmoteDialog bind:mode={reportDialogMode} targetId={data.id} />
	<DeleteEmoteDialog bind:mode={deleteDialogMode} bind:data />
{/if}
{#if data}
	<div class="top-bar">
		<Flags flags={emoteToFlags(data, $defaultEmoteSet, $editableEmoteSets)} />
		{#if data.owner}
			<div class="user-info">
				<a href="/users/{data.owner.id}">
					<UserProfilePicture user={data.owner} />
				</a>
				<div class="name-container">
					<a href="/users/{data.owner.id}" class="username">
						<UserName user={data.owner} />
					</a>
					{#if data.attribution.length > 0}
						<div class="artists">
							<ArrowBendDownRight size="0.75rem" color="var(--text-light)" />
							{#each data.attribution as artist}
								{#if artist.user}
									<a href="/users/{artist.user.id}" class="profile">
										<UserProfilePicture user={artist.user} size={16} />
									</a>
								{/if}
							{/each}
						</div>
					{/if}
				</div>
			</div>
		{/if}
	</div>
{/if}

{#snippet caret()}
	<CaretRight />
{/snippet}

<div class="emote-info">
	<div class="heading">
		{#if data}
			<h1 title={data.defaultName}>{data.defaultName}</h1>
			<Tags tags={data.tags} />
		{:else}
			<Spinner />
		{/if}
	</div>
	<div class="previews">
		{#if data}
			{#if data.imagesPending}
				<div class="processing">
					<Cpu />
					This emote is still processing
					{#if data}
						<button class="refresh" onclick={() => data && invalidate(`emotes:${data.id}`)}>
							Refresh
						</button>
					{/if}
				</div>
			{:else}
				{#each prepareImages(data.images) as group}
					{#if group}
						<EmoteInfoImage images={group} />
					{/if}
				{/each}
			{/if}
		{:else}
			<EmoteLoadingPlaceholder index={0} size={32} />
			<EmoteLoadingPlaceholder index={1} size={64} />
			<EmoteLoadingPlaceholder index={2} size={96} />
			<EmoteLoadingPlaceholder index={3} size={128} />
		{/if}
	</div>
	{#if data}
		<div class="buttons">
			{#snippet fallbackChildren()}
				{#if $user && data !== null && !data.deleted}
					<EmoteUseButton {data} primary />
					<Button
						primary={!$defaultEmoteSet}
						secondary={!!$defaultEmoteSet}
						onclick={() => (addEmoteDialogMode = "shown")}
					>
						{#snippet icon()}
							<FolderPlus />
						{/snippet}
						{$t("pages.emote.add_to")}
					</Button>
					{#if editPermission}
						<Button secondary hideOnMobile onclick={() => (editDialogMode = "shown")}>
							{#snippet icon()}
								<NotePencil />
							{/snippet}
							{$t("labels.edit")}
						</Button>
						<Button secondary hideOnDesktop onclick={() => (editDialogMode = "shown")}>
							{#snippet icon()}
								<NotePencil />
							{/snippet}
						</Button>
					{/if}
				{/if}
				<a href={undefined} style="display: none" bind:this={downloadElement}>Download</a>
				<DropDown bind:this={moreMenuDropdown}>
					{#if !data?.imagesPending && formats && formats.length > 1}
						<Button secondary onclick={() => (moreMenuMode = "root")}>
							{#if $user}
								{$t("labels.more")}
							{:else}
								{$t("labels.actions")}
							{/if}

							{#snippet iconRight()}
								<CaretDown />
							{/snippet}
						</Button>
					{/if}

					{#snippet dropdown()}
						<div class="dropdown">
							{#if moreMenuMode === "root"}
								{#if $user?.permissions.emote.manageAny}
									<MenuButton onclick={() => (transferDialogMode = "shown")}>
										<PaperPlaneRight />
										{$t("pages.emote.transfer")}
									</MenuButton>
								{/if}
								{#if editPermission}
									<MenuButton>
										<ArrowsMerge style="transform: rotate(-90deg)" />
										{$t("pages.emote.merge")}
									</MenuButton>
								{/if}
								{#if !data?.imagesPending && formats && formats.length > 1}
									<MenuButton
										iconRight={caret}
										onclick={() => {
											setDialogState("format", true);
										}}
									>
										<Download />
										{$t("labels.download")}
									</MenuButton>
									<MenuButton
										iconRight={caret}
										onclick={() => {
											setDialogState("format", false);
										}}
									>
										<CopySimple />
										{$t("labels.copy_link")}
									</MenuButton>
								{/if}
								{#if $user?.permissions.ticket.create}
									<MenuButton onclick={() => (reportDialogMode = "shown")}>
										<Flag />
										{$t("labels.report")}
									</MenuButton>
								{/if}
								<hr />
								{#if editPermission}
									<MenuButton
										style="color: var(--danger)"
										onclick={() => (deleteDialogMode = "shown")}
									>
										<Trash />
										{$t("labels.delete")}
									</MenuButton>
								{/if}
							{:else if moreMenuMode === "format"}
								{#each formats ?? [] as format}
									<MenuButton iconRight={caret} onclick={() => clickFormat(format)}>
										{#if format === "image/avif"}
											AVIF
										{:else if format === "image/webp"}
											WebP
										{:else if format === "image/gif"}
											GIF
										{:else if format === "image/png"}
											PNG
										{:else}
											{format}
										{/if}
									</MenuButton>
								{/each}
							{:else if moreMenuMode === "size"}
								{#each sizes ?? [] as image}
									<MenuButton
										onclick={moreMenuAction === "download"
											? () => downloadImage(image)
											: () => copyToClipboard(image.url)}
									>
										{image.scale}x {$t("pages.emote.size")}
									</MenuButton>
								{/each}
							{/if}
						</div>
					{/snippet}
				</DropDown>
			{/snippet}

			{@render (children ?? fallbackChildren)()}
		</div>
	{/if}
</div>

<style lang="scss">
	.top-bar {
		display: flex;
		justify-content: space-between;
		flex-direction: row-reverse;
		align-items: center;
		gap: 1rem;
	}

	.user-info {
		display: flex;
		align-items: center;
		column-gap: 0.5rem;
		text-decoration: none;

		.name-container {
			display: flex;
			flex-direction: column;
			gap: 0.25rem;
		}

		.username {
			font-weight: 500;
			color: var(--text);
			text-decoration: none;
		}

		.artists {
			display: flex;
			gap: 0.25rem;
		}
	}

	.emote-info {
		margin-top: 0.5rem;

		display: flex;
		flex-direction: column;
		justify-content: space-between;
		align-items: center;
		gap: 2rem;

		.heading {
			display: flex;
			flex-direction: column;
			align-items: center;
			gap: 1rem;

			max-width: 90%;

			h1 {
				font-size: 1.25rem;
				font-weight: 600;
				max-width: 100%;
				overflow: hidden;
				text-overflow: ellipsis;
				white-space: nowrap;
			}
		}

		.processing {
			display: flex;
			gap: 0.5rem;

			.refresh:hover,
			.refresh:focus-visible {
				text-decoration: underline;
			}
		}

		.previews {
			display: flex;
			align-items: flex-end;
			gap: 2.25rem;
			overflow: auto;
			max-width: 100%;
		}

		.buttons {
			display: flex;
			gap: 0.5rem;
		}

		.dropdown {
			display: flex;
			flex-direction: column;
		}
	}

	@media screen and (max-width: 960px) {
		.emote-info .previews {
			gap: 0.75rem;
		}
	}
</style>
