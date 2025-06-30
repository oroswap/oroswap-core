import {
    CONTRACTS,
    execContract,
    get_signers,
    getCw20Balance,
    getNativeBalance,
    ibcIcs20Transfer,
    ibcTransferOldOro,
    LCD,
    load_config,
    migrateContract,
    Signer,
    simulateAndBroadcast,
    storeCode,
    TestConfig,
    toBase64
} from "../src/lib";
import {assert, expect} from "chai";
import {it} from "mocha";
import {Coin, MsgSend} from "@terra-money/feather.js";

const {terra, neutron} = get_signers()
const config = load_config()
const NEW_CW20_ICS20_CODE_PATH = `${CONTRACTS}/new_cw20_ics20.wasm`

const wait = async (promise: Promise<any>, timeout?: number): Promise<void> => {
    return (async () => {
        await promise
        return new Promise(resolve => setTimeout(resolve, timeout || 1000))
    })()
}

const migrate_cw20_ics20 = async (config: TestConfig) => {
    let new_code_id = await storeCode(LCD, terra.signer, terra.chain_id, NEW_CW20_ICS20_CODE_PATH)
    return migrateContract(LCD, terra.signer, terra.chain_id, config.cw20_ics20, new_code_id, {})
}

const convert_oro = async (config: TestConfig, signer: Signer, amount: number) => {
    switch (signer.chain_id) {
        case "localterra-1":
            const inner_msg = {
                send: {
                    contract: config.terra_converter!,
                    amount: amount.toString(),
                    msg: toBase64({})
                }
            }
            return execContract(LCD, signer.signer, signer.chain_id, config.oro_token, inner_msg)
        case "localneutron-1":
            return execContract(
                LCD, signer.signer, signer.chain_id,
                config.neutron_converter!,
                {convert: {}},
                [new Coin(config.oro_ibc_denom!, amount)]
            )
        default:
            throw new Error(`Unsupported chainId ${signer.chain_id}`)
    }
}

describe('Disable ORO transfers from Terra', () => {
    before(async function () {
        this.timeout(5000)

        await wait(ibcTransferOldOro(terra, config, 1_000_000_000000, neutron.address))
            .catch(() => {
            })
        await migrate_cw20_ics20(config)
            .catch(() => {
            })
    });

    it('should still be able to bridge ORO back to Terra', async function () {
        this.timeout(5000)

        const balBefore = await getCw20Balance(config, terra.address)
        await wait(ibcTransferOldOro(neutron, config, 100, terra.address), 2000)
        const balAfter = await getCw20Balance(config, terra.address)
        expect(balAfter - balBefore).eq(100)

        // cw20 ORO transfers from Terra disabled
        await ibcTransferOldOro(terra, config, 100, neutron.address)
            .then(() => assert.fail("Should have failed"))
            .catch(() => {
            })
    });
});

describe('Convert old <> new ORO on Terra', () => {
    before(async function () {
        this.timeout(5000)

        // Top up converter contracts
        await wait(ibcIcs20Transfer(
            neutron,
            new Coin(config.oro_tf_denom!, 1_000_000_000000),
            config.new_neutron_channel!,
            config.terra_converter!
        )).catch(() => {
        })

        const msg = new MsgSend(neutron.address, config.neutron_converter!, [new Coin(config.oro_tf_denom!, 1_000_000_000000)])
        await simulateAndBroadcast(LCD, neutron.signer, neutron.chain_id, [msg])
    });

    it('Terra: should be able to convert old ORO to new ORO', async function () {
        const nativeBalBefore = await getNativeBalance(terra.address, config.oro_tf_ibc_denom!);
        const cw20BalBefore = await getCw20Balance(config, terra.address)

        await convert_oro(config, terra, 100)

        const nativeBalAfter = await getNativeBalance(terra.address, config.oro_tf_ibc_denom!);
        const cw20BalAfter = await getCw20Balance(config, terra.address)

        expect(nativeBalAfter - nativeBalBefore).eq(100)
        expect(cw20BalAfter - cw20BalBefore).eq(-100)
    });

    it('Neutron: should be able to convert old ORO to new ORO', async function () {
        const newOroBalBefore = await getNativeBalance(neutron.address, config.oro_tf_denom!);
        const oldBalBefore = await getNativeBalance(neutron.address, config.oro_ibc_denom!)

        await convert_oro(config, neutron, 100)

        const newOroBalAfter = await getNativeBalance(neutron.address, config.oro_tf_denom!);
        const oldBalAfter = await getNativeBalance(neutron.address, config.oro_ibc_denom!)

        expect(newOroBalAfter - newOroBalBefore).eq(100)
        expect(oldBalAfter - oldBalBefore).eq(-100)
    });
})

describe('Transfer old ORO from Neutron', () => {
    before(async function () {
        this.timeout(5000)

        await convert_oro(config, neutron, 100_000)

        // Top up converter contract NTRN balance to be able to dispatch IBC messages
        const msg = new MsgSend(neutron.address, config.neutron_converter!, [new Coin("untrn", 1_000000)])
        await simulateAndBroadcast(LCD, neutron.signer, neutron.chain_id, [msg])
    });

    it('should be able to IBC old ORO to Terra and burn', async function () {
        this.timeout(5000)

        const oldBalBefore = await getNativeBalance(config.neutron_converter!, config.oro_ibc_denom!)
        expect(oldBalBefore).gt(0)

        await wait(execContract(
            LCD, neutron.signer, neutron.chain_id,
            config.neutron_converter!,
            {transfer_for_burning: {}},
        ))

        const oldBalAfter = await getNativeBalance(config.neutron_converter!, config.oro_ibc_denom!)
        expect(oldBalAfter).eq(0)
    });
})

describe('Burn old cw20 ORO on Terra', () => {
    before(async function () {
        this.timeout(10000)

        await convert_oro(config, neutron, 100_000)

        // Top up converter contract NTRN balance to be able to dispatch IBC messages
        const msg = new MsgSend(neutron.address, config.neutron_converter!, [new Coin("untrn", 1_000000)])
        await simulateAndBroadcast(LCD, neutron.signer, neutron.chain_id, [msg])

        // Transfer old ORO to Terra
        await wait(execContract(
            LCD, neutron.signer, neutron.chain_id,
            config.neutron_converter!,
            {transfer_for_burning: {}},
        ), 3000)
    });

    it('should be able to burn CW20 Oro on Terra', async function () {
        const oldBalBefore = await getCw20Balance(config, config.terra_converter!)
        expect(oldBalBefore).gt(0)

        const totalSupplyBefore = await LCD.wasm.contractQuery(config.oro_token, {token_info: {}})
            .then((resp: any) => {
                return parseInt(resp.total_supply)
            })

        await execContract(
            LCD, terra.signer, terra.chain_id,
            config.terra_converter!,
            {burn: {}},
        )

        const oldBalAfter = await getCw20Balance(config, config.terra_converter!)
        expect(oldBalAfter).eq(0)

        // Assert total supply was reduced
        const totalSupplyAfter = await LCD.wasm.contractQuery(config.oro_token, {token_info: {}})
            .then((resp: any) => {
                return parseInt(resp.total_supply)
            })

        // Whole terra converter balance was burned
        expect(totalSupplyBefore - totalSupplyAfter).eq(oldBalBefore)
    });
})