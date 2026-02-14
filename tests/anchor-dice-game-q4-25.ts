import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { AnchorDiceGameQ425 } from "../target/types/anchor_dice_game_q4_25";
import { randomBytes } from "crypto";
import { text } from "stream/consumers";

describe("anchor-dice-game-q4-25", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env()
  anchor.setProvider(provider);
  const connection = provider.connection;

  const program = anchor.workspace.anchorDiceGameQ425 as Program<AnchorDiceGameQ425>;

  let house = anchor.web3.Keypair.generate();
  let player = anchor.web3.Keypair.generate();
  let [vault] = anchor.web3.PublicKey.findProgramAddressSync([Buffer.from("vault"),house.publicKey.toBuffer()],program.programId);
  let seed = new anchor.BN(randomBytes(16));
  let [bet, betBump] = anchor.web3.PublicKey.findProgramAddressSync([Buffer.from("bet"), vault.toBuffer(), seed.toBuffer("le",16)], program.programId)

  it("Airdrop", async () => {
    await Promise.all([house,player].map(async (k)=> {
      await connection.requestAirdrop(k.publicKey,100*anchor.web3.LAMPORTS_PER_SOL)
    }))
  })

  it("Is initialized!", async () => {
    const amount = new anchor.BN(anchor.web3.LAMPORTS_PER_SOL);
    const initTx = await program.methods
    .initialize(amount)
    .accountsStrict({
      house: house.publicKey,
      vault: vault,
      systemProgram: anchor.web3.SystemProgram.programId
    })
    .signers([house])
    .rpc()

    console.log("Init Succesful", initTx);
  });
  it("places a bet", async() => {
    
    const betAmount = new anchor.BN(anchor.web3.LAMPORTS_PER_SOL/100);
    const [betPda] = anchor.web3.PublicKey.findProgramAddressSync([Buffer.from("bet"),vault.toBuffer(),seed.toBuffer("le",16)], program.programId)
    const betTx = await program.methods.placeBet(seed,50,betAmount)
    .accountsStrict({
      player: player.publicKey,
      house: house.publicKey,
      vault: vault,
      bet: betPda,
      systemProgram: anchor.web3.SystemProgram.programId

    })
    .signers([player])
    .rpc()
    console.log("Bet Succesful", betTx);
  })

  it("resolves a bet", async () => {
    const betAccount = (await connection.getAccountInfo(bet,"confirmed"));
    let sig_ix = anchor.web3.Ed25519Program.createInstructionWithPrivateKey({
      privateKey:house.secretKey,
      message: betAccount.data.subarray(8)
    });

    const resolveIx = await program.methods
    .resolveBet(Buffer.from(sig_ix.data.buffer.slice(16+32,16+32+64)))
    .accountsStrict({
      house: house.publicKey,
      player: player.publicKey,
      vault,
      bet,
      instructionSysvar: anchor.web3.SYSVAR_INSTRUCTIONS_PUBKEY,
      systemProgram: program.programId
    })
    .signers([house])
    .instruction();

    const tx = new anchor.web3.Transaction().add(sig_ix).add(resolveIx);

    try {
      const txReciept = await anchor.web3.sendAndConfirmTransaction(connection,tx,[house]);
      console.log("txReciept:" ,txReciept.toString())
    } catch (error) {
      console.log("Error Resolving Bet", error)
    }
  })

  it("refunds a bet", async () => {
    const refundTx = await program.methods.refundBet().accountsStrict({
      player:player.publicKey,
      house:house.publicKey,
      vault:vault,
      bet: bet,
      systemProgram: anchor.web3.SystemProgram.programId
    })
    .signers([player])
    .rpc()

    console.log("Refund Successful", refundTx)
  })
});
