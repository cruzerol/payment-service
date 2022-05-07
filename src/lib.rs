use std::{sync::{Arc, mpsc::{Sender}}, str::FromStr};

use anyhow::{Result, Ok, Error, ensure};
use ethers::prelude::{Address, Provider, Http, Filter, Middleware, ValueOrArray, H256, Log, U256};
use rand::Rng;


#[derive(Debug, Clone)]
pub struct Payment {
    pub amount: f64,
    pub token: Address
}

impl Payment {
    pub fn random_amount(amount: f64) -> f64 {
        let mut random = rand::thread_rng();
        let random: f64 = random.gen();
        let random = random + 0.0001;

        let amount = format!("{:.4}", random + amount).parse::<f64>().unwrap();

        amount
    } 
    fn new(amount: f64, token: Address) -> Self {
        Self {
            amount: Self::random_amount(amount),
            token,
        }
    }
}

pub struct PaymentService {
    provider: Arc<Provider<Http>>,
    payments: Vec<Payment>,
    payment_tokens: Vec<Address>,

    receiver: Address
}

struct FilterBuilder;

impl FilterBuilder {
    pub fn build(start_block: u64, token: Address, receiver: Address) -> Filter {
        Filter::new()
        .address(ValueOrArray::Value(token))
        .from_block(start_block)
        .to_block(start_block + 10)
        .topic0("0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef".parse::<H256>().unwrap())
        .topic2(receiver)
    }
}

impl PaymentService {
    pub fn new(provider: Arc<Provider<Http>>, payment_tokens: Vec<Address>, receiver: Address) -> Self {
        Self {
            provider,
            payments: vec![],
            payment_tokens,
            receiver
        }
    }

    pub fn create_payment(&mut self, amount: f64, token: Address) -> Result<Payment, Error> {
        ensure!(self.payment_tokens.contains(&token), "Token doesn't support");

        let payment = Payment::new(amount, token);

        self.payments.push(payment.clone());
        
        Ok(payment)
    }

    pub async fn serve_log(&self, log: Log) -> usize {
        let amount = U256::from_str(&*log.data.to_string());
        if amount.is_err() {
            return usize::MAX;
        }


        let amount = amount.unwrap().to_string().parse::<f64>();
        if amount.is_err() {
            return usize::MAX;
        }

        let amount = amount.unwrap();
        

        let token = log.address;

        for (idx, payment) in self.payments.iter().enumerate() {
            if payment.token != token && payment.amount != amount {
                continue;
            }
            return idx;
        }

        usize::MAX
    }

    pub async fn run(&mut self, out: Sender<Payment>) {
        loop {
            let start_block = self.provider.get_block_number().await;
            if start_block.is_err() {
                continue;
            }

            let start_block = start_block.unwrap().as_u64();

            for token in self.payment_tokens.iter() {
                let filter = FilterBuilder::build(start_block - 5, token.clone(), self.receiver);
                let logs = self.provider.get_logs(&filter).await;
                if logs.is_err() {
                    continue;
                }

                let logs = logs.unwrap();

                for log in logs {
                    let idx = self.serve_log(log).await;
                    if idx == usize::MAX {
                        continue;
                    }
                    
                    if out.send(self.payments[idx].clone()).is_err() {
                        continue;
                    }

                    self.payments.remove(idx);

                }
            }
        }
    }

}